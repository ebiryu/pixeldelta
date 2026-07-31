//! The HTTP client and retry policy shared by the S3-compatible backend and
//! the GitHub notifier.
//!
//! Using one client for both keeps a single TLS implementation in the build,
//! and keeps the timeout and retry policy in one place rather than repeated
//! at each call site.

use std::time::Duration;

/// Timeout for the whole request: from DNS resolution to the last byte of
/// the response body.
const TIMEOUT_GLOBAL: Duration = Duration::from_secs(60);
/// Timeout for establishing the connection, inside the global timeout.
const TIMEOUT_CONNECT: Duration = Duration::from_secs(10);
/// Wait before each retry. Its length is the retry count, so 3 attempts in
/// total.
const RETRY_WAITS: [Duration; 2] = [Duration::from_millis(500), Duration::from_secs(1)];
/// Idle connections the agent keeps, in total and for one host.
///
/// Both are the number of object requests the storage sends at once, and all
/// of those go to the same host. `ureq` keeps 10 idle connections and 3 per
/// host by default, and closes the rest as each request finishes, so the
/// requests beyond that would open a connection and complete a TLS handshake
/// apiece instead of reusing one.
const MAX_IDLE_CONNECTIONS: usize = crate::storage::REQUEST_CONCURRENCY;

/// What a request came back with.
pub(crate) struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Builds the agent used by both the S3 and GitHub clients.
pub(crate) fn agent() -> ureq::Agent {
    agent_with(TIMEOUT_GLOBAL)
}

/// Builds an agent with a given global timeout, so a test can use one short
/// enough to exercise without waiting on the real timeout.
fn agent_with(timeout_global: Duration) -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            // Statuses outside 2xx are answers this crate reads, not
            // transport failures: the S3 backend reads a 404 as "not
            // stored", GitHub reads a 403 as "refused", and the retry below
            // reads a 5xx as a status rather than an error.
            .http_status_as_error(false)
            .timeout_global(Some(timeout_global))
            .timeout_connect(Some(TIMEOUT_CONNECT))
            .max_idle_connections(MAX_IDLE_CONNECTIONS)
            .max_idle_connections_per_host(MAX_IDLE_CONNECTIONS)
            .build(),
    )
}

/// Sends a request through `send`, retrying with the default waits when the
/// answer is a connection failure or a 5xx.
///
/// `send` performs one attempt; it is called again for each retry, so a
/// caller that signs its request (the S3 backend) signs each attempt anew.
pub(crate) fn retry<E>(send: impl FnMut() -> Result<Answer, E>) -> Result<Answer, E> {
    retry_with(&RETRY_WAITS, send)
}

/// Retries `send` after each wait in `waits`, stopping at the first answer
/// that is not a connection failure or a 5xx. When `waits` runs out, the
/// last result is returned as-is, whatever it was.
fn retry_with<E>(
    waits: &[Duration],
    mut send: impl FnMut() -> Result<Answer, E>,
) -> Result<Answer, E> {
    let mut waits = waits.iter();
    loop {
        let result = send();
        let retryable = match &result {
            Ok(answer) => answer.status >= 500,
            Err(_) => true,
        };
        if !retryable {
            return result;
        }
        match waits.next() {
            Some(wait) => std::thread::sleep(*wait),
            None => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::mpsc;

    fn zero_waits(n: usize) -> Vec<Duration> {
        vec![Duration::ZERO; n]
    }

    /// The pool has to hold every connection the storage keeps in flight, or
    /// the requests past its size open one apiece. Nothing else in a test can
    /// observe that, since it shows up as handshakes rather than as answers.
    #[test]
    fn the_pool_holds_one_connection_per_parallel_request() {
        let config = agent().config().clone();

        assert_eq!(
            config.max_idle_connections_per_host(),
            crate::storage::REQUEST_CONCURRENCY
        );
        assert_eq!(
            config.max_idle_connections(),
            crate::storage::REQUEST_CONCURRENCY
        );
    }

    #[test]
    fn a_200_stops_after_one_attempt() {
        let calls = Cell::new(0);
        let result = retry_with(&zero_waits(2), || {
            calls.set(calls.get() + 1);
            Ok::<Answer, String>(Answer {
                status: 200,
                body: Vec::new(),
            })
        });

        assert_eq!(result.expect("a 200 is returned").status, 200);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn a_4xx_stops_after_one_attempt() {
        let calls = Cell::new(0);
        let result = retry_with(&zero_waits(2), || {
            calls.set(calls.get() + 1);
            Ok::<Answer, String>(Answer {
                status: 403,
                body: Vec::new(),
            })
        });

        assert_eq!(result.expect("a 403 is returned").status, 403);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn a_5xx_is_retried_until_the_waits_run_out() {
        let calls = Cell::new(0);
        let result = retry_with(&zero_waits(2), || {
            calls.set(calls.get() + 1);
            Ok::<Answer, String>(Answer {
                status: 503,
                body: Vec::new(),
            })
        });

        // Two waits mean 3 attempts: the first send plus one per wait.
        assert_eq!(result.expect("the last 5xx is returned").status, 503);
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn an_error_is_retried_and_can_still_succeed() {
        let calls = Cell::new(0);
        let result = retry_with(&zero_waits(2), || {
            let attempt = calls.get();
            calls.set(attempt + 1);
            if attempt == 0 {
                Err("connection reset".to_owned())
            } else {
                Ok(Answer {
                    status: 200,
                    body: Vec::new(),
                })
            }
        });

        assert_eq!(result.expect("the retry succeeds").status, 200);
        assert_eq!(calls.get(), 2);
    }

    /// With no timeout set, a stalled response would hang forever, so `call`
    /// runs on its own thread and the test's own wait is what bounds it.
    #[test]
    fn a_global_timeout_stops_a_stalled_response() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
        let addr = listener.local_addr().expect("the bound address");
        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                // Hold the connection open without writing a response, so
                // the global timeout is what ends the call.
                std::thread::sleep(Duration::from_secs(5));
                drop(stream);
            }
        });

        let agent = agent_with(Duration::from_millis(300));
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = agent.get(format!("http://{addr}")).call();
            let _ = sender.send(result.is_err());
        });

        let timed_out = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("the call finishes inside the global timeout, not the test's own wait");
        assert!(timed_out, "a stalled response should end in an error");
    }
}

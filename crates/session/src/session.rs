use db::kvp::KeyValueStore;
use gpui::{App, AppContext as _, Context, Subscription, Task, WindowId};
use util::ResultExt;

pub struct Session {
    session_id: String,
    old_session_id: Option<String>,
    old_window_ids: Option<Vec<WindowId>>,
    had_abnormal_exit: bool,
}

const SESSION_ID_KEY: &str = "session_id";
const SESSION_WINDOW_STACK_KEY: &str = "session_window_stack";
const CLEAN_EXIT_KEY: &str = "clean_exit";

impl Session {
    pub async fn new(session_id: String, db: KeyValueStore) -> Self {
        let old_session_id = db.read_kvp(SESSION_ID_KEY).ok().flatten();
        let clean_exit = db.read_kvp(CLEAN_EXIT_KEY).ok().flatten();

        let had_abnormal_exit =
            old_session_id.is_some() && clean_exit.as_deref() == Some("false");

        db.write_kvp(SESSION_ID_KEY.to_string(), session_id.clone())
            .await
            .log_err();

        db.write_kvp(CLEAN_EXIT_KEY.to_string(), "false".to_string())
            .await
            .log_err();

        let old_window_ids = db
            .read_kvp(SESSION_WINDOW_STACK_KEY)
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<Vec<u64>>(&json).ok())
            .map(|vec: Vec<u64>| {
                vec.into_iter()
                    .map(WindowId::from)
                    .collect::<Vec<WindowId>>()
            });

        Self {
            session_id,
            old_session_id,
            old_window_ids,
            had_abnormal_exit,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn test() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            old_session_id: None,
            old_window_ids: None,
            had_abnormal_exit: false,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn test_with_old_session(old_session_id: String) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            old_session_id: Some(old_session_id),
            old_window_ids: None,
            had_abnormal_exit: false,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn test_with_abnormal_exit(old_session_id: String) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            old_session_id: Some(old_session_id),
            old_window_ids: None,
            had_abnormal_exit: true,
        }
    }

    pub fn id(&self) -> &str {
        &self.session_id
    }

    pub fn had_abnormal_exit(&self) -> bool {
        self.had_abnormal_exit
    }

    pub fn old_session_id(&self) -> Option<&str> {
        self.old_session_id.as_deref()
    }
}

pub struct AppSession {
    session: Session,
    _serialization_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl AppSession {
    pub fn new(session: Session, cx: &Context<Self>) -> Self {
        let _subscriptions = vec![cx.on_app_quit(Self::app_will_quit)];

        let _serialization_task = if cfg!(not(any(test, feature = "test-support"))) {
            let db = KeyValueStore::global(cx);
            cx.spawn(async move |_, cx| {
                // Disabled in tests: the infinite loop bypasses "parking forbidden" checks,
                // causing tests to hang instead of panicking.
                {
                    let mut current_window_stack = Vec::new();
                    loop {
                        if let Some(windows) = cx.update(|cx| window_stack(cx))
                            && windows != current_window_stack
                        {
                            store_window_stack(db.clone(), &windows).await;
                            current_window_stack = windows;
                        }

                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(500))
                            .await;
                    }
                }
            })
        } else {
            Task::ready(())
        };

        Self {
            session,
            _subscriptions,
            _serialization_task,
        }
    }

    fn app_will_quit(&mut self, cx: &mut Context<Self>) -> Task<()> {
        let db = KeyValueStore::global(cx);
        let window_stack = window_stack(cx);
        cx.background_spawn(async move {
            if let Some(window_stack) = window_stack {
                store_window_stack(db.clone(), &window_stack).await;
            }
            db.write_kvp(CLEAN_EXIT_KEY.to_string(), "true".to_string())
                .await
                .log_err();
        })
    }

    pub fn id(&self) -> &str {
        self.session.id()
    }

    pub fn had_abnormal_exit(&self) -> bool {
        self.session.had_abnormal_exit()
    }

    pub fn last_session_id(&self) -> Option<&str> {
        self.session.old_session_id.as_deref()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn replace_session_for_test(&mut self, session: Session) {
        self.session = session;
    }

    pub fn last_session_window_stack(&self) -> Option<Vec<WindowId>> {
        self.session.old_window_ids.clone()
    }
}

fn window_stack(cx: &App) -> Option<Vec<u64>> {
    Some(
        cx.window_stack()?
            .into_iter()
            .map(|window| window.window_id().as_u64())
            .collect(),
    )
}

async fn store_window_stack(db: KeyValueStore, windows: &[u64]) {
    if let Ok(window_ids_json) = serde_json::to_string(windows) {
        db.write_kvp(SESSION_WINDOW_STACK_KEY.to_string(), window_ids_json)
            .await
            .log_err();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    async fn test_abnormal_exit_detection() {
        let db = KeyValueStore::open_test_db("test_session_abnormal_exit").await;

        let id1 = uuid::Uuid::new_v4().to_string();
        let session1 = Session::new(id1.clone(), db.clone()).await;
        assert!(!session1.had_abnormal_exit());

        let id2 = uuid::Uuid::new_v4().to_string();
        let session2 = Session::new(id2.clone(), db.clone()).await;
        assert!(session2.had_abnormal_exit());
        assert_eq!(session2.old_session_id(), Some(id1.as_str()));

        db.write_kvp(CLEAN_EXIT_KEY.to_string(), "true".to_string())
            .await
            .unwrap();

        let id3 = uuid::Uuid::new_v4().to_string();
        let session3 = Session::new(id3.clone(), db.clone()).await;
        assert!(!session3.had_abnormal_exit());
        assert_eq!(session3.old_session_id(), Some(id2.as_str()));
    }
}



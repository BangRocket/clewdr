use std::collections::VecDeque;

use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use snafu::{GenerateImplicitData, Location};
use tracing::{info, warn};

use crate::config::{CodexAuth, CLEWDR_CONFIG, ClewdrConfig};
use crate::error::ClewdrError;

#[derive(Debug)]
enum Msg {
    Request(RpcReplyPort<Result<CodexAuth, ClewdrError>>),
    Return(CodexAuth),
    Submit(CodexAuth, RpcReplyPort<Result<(), ClewdrError>>),
    Delete(String, RpcReplyPort<Result<(), ClewdrError>>),
    List(RpcReplyPort<Vec<CodexAuth>>),
}

#[derive(Debug)]
struct State {
    pool: VecDeque<CodexAuth>,
}

struct CodexAuthActor;

impl CodexAuthActor {
    fn dispatch(state: &mut State) -> Result<CodexAuth, ClewdrError> {
        let now = chrono::Utc::now().timestamp();
        let n = state.pool.len();
        for _ in 0..n {
            let Some(mut c) = state.pool.pop_front() else {
                break;
            };
            if c.is_dispatchable(now) {
                c.last_used_at = Some(now);
                let returning = c.clone();
                state.pool.push_back(c);
                return Ok(returning);
            }
            // not dispatchable — keep in pool but rotate
            state.pool.push_back(c);
        }
        Err(ClewdrError::NoCodexAuthAvailable)
    }

    fn collect(state: &mut State, returned: CodexAuth) {
        if let Some(idx) = state.pool.iter().position(|x| x.id == returned.id) {
            state.pool[idx] = returned;
        } else {
            state.pool.push_back(returned);
        }
        Self::persist(state);
    }

    fn persist(state: &State) {
        let snapshot: Vec<CodexAuth> = state.pool.iter().cloned().collect();
        CLEWDR_CONFIG.rcu(|cfg| {
            let mut cfg = ClewdrConfig::clone(cfg);
            cfg.codex_auth = snapshot.clone();
            cfg
        });
        tokio::spawn(async move {
            if let Err(e) = CLEWDR_CONFIG.load().save().await {
                warn!("codex_auth save failed: {e}");
            }
        });
    }
}

impl Actor for CodexAuthActor {
    type Msg = Msg;
    type State = State;
    type Arguments = Vec<CodexAuth>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        seed: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!("CodexAuthActor starting with {} creds", seed.len());
        Ok(State {
            pool: VecDeque::from(seed),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::Request(reply) => {
                let r = Self::dispatch(state);
                reply.send(r)?;
            }
            Msg::Return(c) => {
                Self::collect(state, c);
            }
            Msg::Submit(c, reply) => {
                if state.pool.iter().any(|x| x.id == c.id) {
                    reply.send(Err(ClewdrError::BadRequest {
                        msg: "duplicate codex auth id",
                    }))?;
                } else {
                    state.pool.push_back(c);
                    Self::persist(state);
                    reply.send(Ok(()))?;
                }
            }
            Msg::Delete(id, reply) => {
                let before = state.pool.len();
                state.pool.retain(|x| x.id != id);
                if state.pool.len() < before {
                    Self::persist(state);
                    reply.send(Ok(()))?;
                } else {
                    reply.send(Err(ClewdrError::UnexpectedNone {
                        msg: "codex auth id not found in pool",
                    }))?;
                }
            }
            Msg::List(reply) => {
                reply.send(state.pool.iter().cloned().collect())?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct CodexAuthActorHandle {
    actor_ref: ActorRef<Msg>,
}

impl CodexAuthActorHandle {
    pub async fn start() -> Result<Self, ractor::SpawnErr> {
        let seed = CLEWDR_CONFIG.load().codex_auth.clone();
        Self::start_with(seed).await
    }

    pub async fn start_with(seed: Vec<CodexAuth>) -> Result<Self, ractor::SpawnErr> {
        let (actor_ref, _) = Actor::spawn(None, CodexAuthActor, seed).await?;
        Ok(Self { actor_ref })
    }

    pub async fn request(&self) -> Result<CodexAuth, ClewdrError> {
        ractor::call!(self.actor_ref, Msg::Request).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth request: {e}"),
        })?
    }

    pub async fn return_auth(&self, auth: CodexAuth) -> Result<(), ClewdrError> {
        ractor::cast!(self.actor_ref, Msg::Return(auth)).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth return: {e}"),
        })
    }

    pub async fn submit(&self, auth: CodexAuth) -> Result<(), ClewdrError> {
        ractor::call!(self.actor_ref, Msg::Submit, auth).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth submit: {e}"),
        })?
    }

    pub async fn delete(&self, id: String) -> Result<(), ClewdrError> {
        ractor::call!(self.actor_ref, Msg::Delete, id).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth delete: {e}"),
        })?
    }

    pub async fn list(&self) -> Result<Vec<CodexAuth>, ClewdrError> {
        ractor::call!(self.actor_ref, Msg::List).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth list: {e}"),
        })
    }
}

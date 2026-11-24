use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Default)]
pub struct OperationGate(Mutex<Option<OperationIdentity>>);

impl OperationGate {
    pub fn activate(&self, operation: OperationIdentity) -> Result<(), AppError> {
        let mut active = self.0.lock().expect("assistant active mutex poisoned");
        if active.is_some() {
            return Err(AppError::new("ASSISTANT_TURN_BUSY", "已有智奈分析正在运行"));
        }
        *active = Some(operation);
        Ok(())
    }

    pub fn clear(&self) {
        *self.0.lock().expect("assistant active mutex poisoned") = None;
    }

    pub fn accepts(&self, operation: &OperationIdentity) -> bool {
        self.0
            .lock()
            .expect("assistant active mutex poisoned")
            .as_ref()
            == Some(operation)
    }

    pub fn is_active(&self) -> bool {
        self.0
            .lock()
            .expect("assistant active mutex poisoned")
            .is_some()
    }

    pub fn active(&self) -> Option<OperationIdentity> {
        self.0
            .lock()
            .expect("assistant active mutex poisoned")
            .clone()
    }

    pub fn claim_completion(&self, operation: &OperationIdentity) -> bool {
        let mut active = self.0.lock().expect("assistant active mutex poisoned");
        if active.as_ref() != Some(operation) {
            return false;
        }
        *active = None;
        true
    }

    pub fn cancel_active(&self) -> Option<OperationIdentity> {
        self.0
            .lock()
            .expect("assistant active mutex poisoned")
            .take()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationIdentity {
    pub request_id: String,
    pub conversation_id: String,
    pub session_id: String,
    pub operation_id: String,
}

impl OperationIdentity {
    pub fn new(
        request_id: impl Into<String>,
        conversation_id: impl Into<String>,
        session_id: impl Into<String>,
        operation_id: impl Into<String>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            conversation_id: conversation_id.into(),
            session_id: session_id.into(),
            operation_id: operation_id.into(),
        }
    }
}

pub trait BridgeRuntime: Send {
    fn interrupt_handle(&self) -> Arc<dyn BridgeRuntimeInterrupt>;
    fn start(&mut self) -> Result<(), AppError>;
    fn stop(&mut self) -> Result<(), AppError>;
    fn run(&mut self, operation: &OperationIdentity, input: &str) -> Result<String, AppError>;
    fn configure(
        &mut self,
        provider: String,
        model: String,
        profile_patch: Option<String>,
    ) -> Result<(), AppError>;
}

pub trait BridgeRuntimeInterrupt: Send + Sync {
    fn interrupt(&self) -> Result<(), AppError>;
}

struct SupervisorState<R> {
    runtime: R,
    started: bool,
    recovered: bool,
}

pub struct AssistantSupervisor<R> {
    state: Mutex<SupervisorState<R>>,
    gate: Arc<OperationGate>,
    interrupt: Arc<dyn BridgeRuntimeInterrupt>,
}

impl<R: BridgeRuntime> AssistantSupervisor<R> {
    pub fn new(runtime: R) -> Self {
        Self::with_gate(runtime, Arc::new(OperationGate::default()))
    }

    pub fn with_gate(runtime: R, gate: Arc<OperationGate>) -> Self {
        let interrupt = runtime.interrupt_handle();
        Self {
            state: Mutex::new(SupervisorState {
                runtime,
                started: false,
                recovered: false,
            }),
            gate,
            interrupt,
        }
    }

    pub fn run(&self, operation: OperationIdentity, input: &str) -> Result<String, AppError> {
        let result = self.run_claimable(operation.clone(), input);
        if result.is_ok() && !self.claim_completion(&operation) {
            return Err(AppError::new("ASSISTANT_STOPPED", "智奈分析已停止"));
        }
        result
    }

    pub fn run_claimable(
        &self,
        operation: OperationIdentity,
        input: &str,
    ) -> Result<String, AppError> {
        self.gate.activate(operation.clone())?;
        let result = self.run_active(&operation, input);
        if result.is_err() {
            let _ = self.gate.claim_completion(&operation);
        }
        result
    }

    fn run_active(&self, operation: &OperationIdentity, input: &str) -> Result<String, AppError> {
        let mut state = self.state.lock().expect("assistant state mutex poisoned");
        if !state.started {
            state.runtime.start()?;
            state.started = true;
        }
        let result = state.runtime.run(operation, input);
        if result.is_err() && !state.recovered {
            let _ = state.runtime.stop();
            state.started = false;
            state.runtime.start()?;
            state.started = true;
            state.recovered = true;
        }
        result
    }

    pub fn stop(&self) -> Result<(), AppError> {
        self.gate.clear();
        self.interrupt.interrupt()?;
        let mut state = self.state.lock().expect("assistant state mutex poisoned");
        state.runtime.stop()?;
        state.started = false;
        self.gate.clear();
        Ok(())
    }

    pub fn restart(&self) -> Result<(), AppError> {
        let mut state = self.state.lock().expect("assistant state mutex poisoned");
        if state.started {
            state.runtime.stop()?;
        }
        state.runtime.start()?;
        state.started = true;
        state.recovered = false;
        Ok(())
    }

    pub fn configure(
        &self,
        provider: String,
        model: String,
        profile_patch: Option<String>,
    ) -> Result<(), AppError> {
        if self.gate.is_active() {
            return Err(AppError::new("ASSISTANT_TURN_BUSY", "已有智奈分析正在运行"));
        }
        let mut state = self.state.lock().expect("assistant state mutex poisoned");
        if state.started {
            state.runtime.stop()?;
            state.started = false;
        }
        state.runtime.configure(provider, model, profile_patch)?;
        state.runtime.start()?;
        state.started = true;
        state.recovered = false;
        Ok(())
    }

    pub fn accepts_event(&self, operation: &OperationIdentity) -> bool {
        self.gate.accepts(operation)
    }

    pub fn active_operation(&self) -> Option<OperationIdentity> {
        self.gate.active()
    }

    pub fn cancel_active_operation(&self) -> Option<OperationIdentity> {
        self.gate.cancel_active()
    }

    pub fn claim_completion(&self, operation: &OperationIdentity) -> bool {
        self.gate.claim_completion(operation)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc, Condvar, Mutex},
        time::Duration,
    };

    use crate::{
        assistant::supervisor::{
            AssistantSupervisor, BridgeRuntime, BridgeRuntimeInterrupt, OperationIdentity,
        },
        error::AppError,
    };

    #[derive(Default)]
    struct RuntimeState {
        starts: usize,
        stops: usize,
        fail_next: bool,
        blocked: bool,
    }

    #[derive(Clone, Default)]
    struct FakeRuntime(Arc<(Mutex<RuntimeState>, Condvar)>);

    impl BridgeRuntimeInterrupt for FakeRuntime {
        fn interrupt(&self) -> Result<(), AppError> {
            let (lock, wake) = &*self.0;
            let mut state = lock.lock().unwrap();
            state.blocked = false;
            wake.notify_all();
            Ok(())
        }
    }

    impl BridgeRuntime for FakeRuntime {
        fn interrupt_handle(&self) -> Arc<dyn BridgeRuntimeInterrupt> {
            Arc::new(self.clone())
        }

        fn start(&mut self) -> Result<(), AppError> {
            self.0 .0.lock().unwrap().starts += 1;
            Ok(())
        }

        fn stop(&mut self) -> Result<(), AppError> {
            self.0 .0.lock().unwrap().stops += 1;
            Ok(())
        }

        fn run(
            &mut self,
            _operation: &OperationIdentity,
            _input: &str,
        ) -> Result<String, AppError> {
            let (lock, wake) = &*self.0;
            let mut state = lock.lock().unwrap();
            if state.fail_next {
                state.fail_next = false;
                return Err(AppError::new("ASSISTANT_RUNTIME_EXITED", "Bridge 异常退出"));
            }
            while state.blocked {
                state = wake.wait(state).unwrap();
            }
            Ok("完成".into())
        }

        fn configure(
            &mut self,
            _provider: String,
            _model: String,
            _profile_patch: Option<String>,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn operation(id: &str) -> OperationIdentity {
        OperationIdentity::new("request", "conversation", "session", id)
    }

    #[test]
    fn allows_only_one_active_turn_and_rejects_late_events() {
        let runtime = FakeRuntime::default();
        runtime.0 .0.lock().unwrap().blocked = true;
        let supervisor = Arc::new(AssistantSupervisor::new(runtime.clone()));
        let running = supervisor.clone();
        let thread = std::thread::spawn(move || running.run(operation("one"), "input"));
        while !supervisor.accepts_event(&operation("one")) {
            std::thread::yield_now();
        }

        assert_eq!(
            supervisor.run(operation("two"), "input").unwrap_err().code,
            "ASSISTANT_TURN_BUSY"
        );
        assert!(!supervisor.accepts_event(&operation("two")));

        let (lock, wake) = &*runtime.0;
        lock.lock().unwrap().blocked = false;
        wake.notify_all();
        thread.join().unwrap().unwrap();
        assert!(!supervisor.accepts_event(&operation("one")));
    }

    #[test]
    fn restarts_once_after_an_unexpected_runtime_exit() {
        let runtime = FakeRuntime::default();
        runtime.0 .0.lock().unwrap().fail_next = true;
        let supervisor = AssistantSupervisor::new(runtime.clone());

        assert_eq!(
            supervisor.run(operation("one"), "input").unwrap_err().code,
            "ASSISTANT_RUNTIME_EXITED"
        );
        let state = runtime.0 .0.lock().unwrap();
        assert_eq!(state.starts, 2);
        assert_eq!(state.stops, 1);
    }

    #[test]
    fn stop_interrupts_an_active_turn_without_waiting_for_it_to_finish() {
        let runtime = FakeRuntime::default();
        runtime.0 .0.lock().unwrap().blocked = true;
        let supervisor = Arc::new(AssistantSupervisor::new(runtime));
        let running = Arc::clone(&supervisor);
        let run_thread = std::thread::spawn(move || running.run(operation("one"), "input"));
        while !supervisor.accepts_event(&operation("one")) {
            std::thread::yield_now();
        }

        let stopping = Arc::clone(&supervisor);
        let (stopped_tx, stopped_rx) = mpsc::channel();
        std::thread::spawn(move || {
            stopped_tx.send(stopping.stop()).unwrap();
        });

        assert!(stopped_rx.recv_timeout(Duration::from_millis(100)).is_ok());
        assert_eq!(
            run_thread.join().unwrap().unwrap_err().code,
            "ASSISTANT_STOPPED"
        );
    }

    #[test]
    fn stopped_operations_cannot_claim_a_late_success() {
        let supervisor = AssistantSupervisor::new(FakeRuntime::default());
        let active = operation("one");

        assert_eq!(
            supervisor.run_claimable(active.clone(), "input").unwrap(),
            "完成"
        );
        supervisor.stop().unwrap();

        assert!(!supervisor.claim_completion(&active));
    }
}

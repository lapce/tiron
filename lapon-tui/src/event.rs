use lapon_common::action::ActionMessage;
use uuid::Uuid;

pub enum AppEvent {
    UserInput(UserInputEvent),
    Run(RunEvent),
    Action {
        run: Uuid,
        host: Uuid,
        msg: ActionMessage,
    },
}

pub enum UserInputEvent {
    ScrollUp,
    ScrollDown,
    Quit,
}

pub enum RunEvent {
    RunStarted { id: Uuid },
    RunCompleted { id: Uuid },
}

extern crate crux;
#[macro_use]
extern crate log;
extern crate env_logger;

use std::fmt::Debug;
use std::time::Duration;
use std::thread;
use crux::State;
use crux::Store;
use crux::Middleware;

#[derive(Debug, Copy, Clone)]
enum TestAction {
    Add(i32),
    Increment,
    Decrement,
    BonusTime, // 20ms Bonus Time
    AssertEq(i32),
}

#[derive(Debug, Clone)]
struct TestState {
    pub number: i32,
}

impl State for TestState {
    type Action = TestAction;

    fn reduce(&mut self, action: TestAction) {
        match action {
            TestAction::Add(x) => self.number += x,
            TestAction::Increment => self.number += 1,
            TestAction::Decrement => self.number -= 1,
            _ => {},
        }
    }
}

impl Log for TestState {
    fn log(&self) -> String {
        format!("{:?}", self)
    }
}

struct BonusTimeMiddleware {
    pub counter: i32,
}

impl Middleware<TestState> for BonusTimeMiddleware {
    fn dispatch(&mut self, store: &Store<TestState>, next: &mut FnMut(TestAction), action: TestAction) {
        if let TestAction::BonusTime = action {
            self.counter += 1;

            let counter = self.counter;
            let mut store_mut = store.clone();

            thread::spawn(move || {
                let number = store_mut.state().number;
                thread::sleep(Duration::from_millis(20));
                let next_number = store_mut.state().number;
                let bonus = (next_number - number) * counter;
                store_mut.dispatch(TestAction::Add(bonus));
            });
            thread::sleep(Duration::from_millis(1));
        }
        next(action);
    }
}

struct AssertMiddleware;

impl Middleware<TestState> for AssertMiddleware {
    fn dispatch(&mut self, store: &Store<TestState>, next: &mut FnMut(TestAction), action: TestAction) {
        if let TestAction::AssertEq(number) = action {
            assert_eq!(store.state().number, number);
        }
        next(action);
    }
}

trait Log {
    fn log(&self) -> String;
}

struct Logger;
impl <T> Middleware<T> for Logger where
    T: State + Log + Clone,
    T::Action: Debug + Copy {
    fn dispatch(&mut self, store: &Store<T>, next: &mut FnMut(T::Action), action: T::Action) {
        debug!("previous state: {}", store.state().log());
        next(action);
        debug!("action: {:?}", action);
        debug!("next state: {}", store.state().log());
    }
}

#[test]
fn store() {
    env_logger::init().unwrap();

    let state = TestState {
        number: 0,
    };
    let mut store = Store::new(state);

    let bonus_time_middleware = BonusTimeMiddleware {
        counter: 0,
    };
    let logger = Logger;
    let assert_middleware = AssertMiddleware;
    store.add_middleware(bonus_time_middleware);
    store.add_middleware(logger);
    store.add_middleware(assert_middleware);

    store.dispatch(TestAction::AssertEq(0));

    store.dispatch(TestAction::Increment);
    store.dispatch(TestAction::AssertEq(1));

    store.dispatch(TestAction::Add(2));
    store.dispatch(TestAction::AssertEq(3));

    // start BonusTime 1
    store.dispatch(TestAction::BonusTime);

    store.dispatch(TestAction::Add(3));
    store.dispatch(TestAction::AssertEq(6));

    store.dispatch(TestAction::Decrement);
    store.dispatch(TestAction::AssertEq(5));

    // finish BonusTime 1
    thread::sleep(Duration::from_millis(25));
    store.dispatch(TestAction::AssertEq(5 + (5 - 3) * 1)); // 7

    // start BonusTime 2
    store.dispatch(TestAction::BonusTime);

    store.dispatch(TestAction::Add(-4));
    store.dispatch(TestAction::AssertEq(3));

    store.dispatch(TestAction::Increment);
    store.dispatch(TestAction::AssertEq(4));

    // finish BonusTime 2
    thread::sleep(Duration::from_millis(25));
    store.dispatch(TestAction::AssertEq(4 + (4 - 7) * 2)); // -2
}

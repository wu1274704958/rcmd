use terminal::{Action, Clear, error, Retrieved, Value};
use std::io::Write;
use std::thread::{Thread, sleep};
use tokio::time::Duration;

pub fn main() -> error::Result<()> {
    let mut out = terminal::stdout();
    // perform an single action.
    out.act(Action::ClearTerminal(Clear::All))?;

    // batch multiple actions.
    for i in 0..20 {
        out.batch(Action::MoveCursorTo(0, 0))?;
        out.write(format!("{}", i).as_bytes());
        out.flush_batch();
        sleep(Duration::from_millis(100));
    }

    // execute batch.
    out.flush_batch();

    // get an terminal value.
    if let Retrieved::TerminalSize(x, y) = out.get(Value::TerminalSize)? {
        println!("\nx: {}, y: {}", x, y);
    }

    Ok(())
}
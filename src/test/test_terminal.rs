use terminal::{Action, Clear, error, Retrieved, Value};
use std::io::Write;
use std::thread::{Thread, sleep};
use tokio::time::Duration;

pub fn main() -> error::Result<()> {
    let mut terminal = terminal::stdout();

    // perform an single action.
    terminal.act(Action::ClearTerminal(Clear::All))?;

    // batch multiple actions.
    for i in 0..20 {
        terminal.batch(Action::MoveCursorTo(0, 0))?;
        terminal.write(format!("{}", i).as_bytes());
        terminal.flush_batch();
        sleep(Duration::from_millis(100));
    }

    // execute batch.
    terminal.flush_batch();

    // get an terminal value.
    if let Retrieved::TerminalSize(x, y) = terminal.get(Value::TerminalSize)? {
        println!("\nx: {}, y: {}", x, y);
    }

    Ok(())
}
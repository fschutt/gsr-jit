#[start]
fn something() -> GameResult {
    GameResult::SendMsg(String::from("hello"))
}
#[allow(unused)]
enum Message {
    Hello,
    Head { head_hash: Vec<u8> },
    ReplayAll,
    ReplayFrom { head_hash: Vec<u8> },
}

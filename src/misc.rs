use tokio_tungstenite::tungstenite;

pub fn handle_result<T>(res: tungstenite::Result<T>, when: impl AsRef<str>) {
    if let Err(err) = res {
        log::warn!("Error occurred when {}: {:?}", when.as_ref(), err);
    }
}

use alpha_bar::net::{dev_bytes, rate};

#[test]
fn proc_net_dev() {
  let dev = "Inter-| Receive | Transmit\n  face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n  wlan0: 1000 0 0 0 0 0 0 0 9000 0 0 0 0 0 0 0\n";
  assert_eq!(dev_bytes(dev, "wlan0"), Some((1000, 9000)));
}

#[test]
fn net_rate() {
  assert_eq!(rate((1000, 9000), (3048, 11048), 1_000_000), (2048, 2048));
}

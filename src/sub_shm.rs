use zenoh::Wait;
fn main() {
    let z = zenoh::open(zenoh::Config::default())
        .wait()
        .expect("Failed to open Zenoh session");    

    let sub = z.declare_subscriber("zenoh/shm/buffer")
        .wait()
        .expect("Failed to declare subscriber");

    while let Ok(s) = sub.recv() {
        let buf = s.payload();
        
        let is_shm = buf.as_shm().is_some();            

        buf.try_to_string()
            .map(|s| println!("Received (SHM: {is_shm}): {s}"))
            .unwrap_or_else(|_| println!("Received non-string payload"));                       
    }
}
use serde_json::json;

pub trait ZConfig {
    fn listen(&mut self,  endpoints: Vec<String>);
    fn connect(&mut self, endpoints: Vec<String>);
}

impl ZConfig for zenoh::Config {
    fn listen(&mut self,  endpoints: Vec<String>) {
        
        let cfg = json!(endpoints).to_string();
        println!("listen/endpoints: {cfg}");
        self.insert_json5("listen/endpoints", &cfg).unwrap()
    }

    fn connect(&mut self, endpoints: Vec<String>) { 
        let cfg = json!(endpoints).to_string();
        println!("connect/endpoints: {cfg}");
        self.insert_json5("connect/endpoints", &cfg).unwrap()
    }
}
use std::io::{BufRead, BufReader, Write};
use std::iter::Iterator;
use std::net::TcpStream;
use std::str::FromStr;
use std::fs::read_to_string;

const SERVER_ADDR: &str = "151.216.74.213:4000";
const USERNAME: &str = "MASTER CONTROL PROGRAM";
const DEBUG: bool = true;

struct Game {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    read_buf: String,
    // blocked fields on the grid
    blocked: Vec<(u8, u8)>,
    // our ID - None means we don't know our ID yet
    me: Option<u8>,
    // other players - ID and name
    others: Vec<(u8, String)>,
}

impl Game {
    fn new() -> Self {
        // connect to server
        println!("Connecting to server");
        let stream = TcpStream::connect(SERVER_ADDR).expect("Cannot connect to server");
        let r = BufReader::new(stream.try_clone().expect("Cannot clone TCPStream"));

        // return game object
        Game {
            reader: r,
            writer: stream,
            read_buf: String::with_capacity(256),
            blocked: Vec::new(),
            me: None,
            others: Vec::new(),
        }
    }

    fn join(&mut self, user: &str, pas: &str) {
        println!("Sending JOIN to join next game");
        self.send("join", Some(&[user, pas]));
    }

    fn reset(&mut self) {
        self.read_buf.clear();
        self.blocked.clear();
        self.me = None;
        self.others.clear()
    }

    fn send(&mut self, msg_type: &str, msg_args: Option<&[&str]>) {
        let msg = match msg_args {
            None => msg_type.into(),
            Some(a) => format!("{}|{}\n", msg_type, a.join("|")),
        };
        if DEBUG {
            println!("Sending msg: {}", msg.trim());
        }
        self.writer
            .write_all(msg.as_bytes())
            .unwrap_or_else(|_| panic!("Failed sending message to server: {}", msg));
        self.writer.flush().expect("Failed flushing");
    }

    fn receive(&mut self) -> &String {
        self.read_buf.clear();
        self.reader
            .read_line(&mut self.read_buf)
            .expect("Cannot read line from server");
        if !self.read_buf.is_empty() && DEBUG {
            println!("Received message: {}", self.read_buf.trim());
        }
        &self.read_buf
    }

    fn add_player(&mut self, id: u8, name: String) {
        if name == USERNAME {
            println!("Found my ID: {}", id);
            self.me = Some(id);
        } else if !self.others.iter().any(|id_name| id_name.0 == id) {
            self.others.push((id, name));
        }
    }

    fn block(&mut self, x: u8, y: u8) {
        // TODO maybe check whether it's already blocked (should be unnecessary though)
        self.blocked.push((x, y));
    }

    fn say(&mut self, msg: &str) {
        self.send("chat", Some(&[msg]));
    }
}

fn parse_msg_arg<T: FromStr>(arg: &str, err_msg: &str) -> T {
    let arg = arg.trim();
    arg.parse()
        .unwrap_or_else(|_| panic!("{}: \"{}\"", err_msg, arg))
}

fn main() {
    // read username from file
    let un_file = "./username";
    let username = read_to_string(un_file).unwrap_or(String::from(USERNAME));
    let username = username.trim();
    println!("I am {}!", username);

    // read password from file
    let pw_file = "./password";
    let password = read_to_string(pw_file)
        .unwrap_or_else(|_| panic!("Cannot read password from file: \"{}\"", pw_file));
    let password = password.trim();

    // connect to server
    let mut game = Game::new();

    // join next game
    game.join(USERNAME, password);

    // count empty messages
    let mut empty_msgs = 0;

    // read loop
    loop {
        // read from server
        let msg = game.receive();

        // ignore empty messages
        if msg.is_empty() {
            empty_msgs += 1;
            continue;
        }
        if empty_msgs > 0 {
            println!("Got {} empty messages so far", empty_msgs);
        }

        // parse message
        let msg_args: Vec<&str> = msg.split('|').collect();
        let msg_type: &str = msg_args[0].trim();

        // decide action
        match msg_type {
            // error - bail out
            "error" => return,

            // MOTD - print
            "motd" => {
                println!("MOTD: {}", msg_args[1].trim());
            }

            // new game - reset game state
            "game" => {
                println!("\nNew game has started!");
                game.reset();
                game.say("You shouldn't have come back, Flynn.");
            }

            // tick - make a move
            "tick" => {
                game.send("move", Some(&["up"]));
            }

            // register players
            "player" => {
                let id = parse_msg_arg(msg_args[1], "Cannot parse to number");
                let name = String::from(msg_args[2].trim());
                println!("Registering player {} \"{}\"", id, name);
                game.add_player(id, name);
            }

            // update blocked fields
            "pos" => {
                let x = parse_msg_arg(msg_args[1], "Cannot parse position (x)");
                let y = parse_msg_arg(msg_args[2], "Cannot parse position (y)");
                game.block(x, y);
            }

            // log chat messages
            "chat" => {
                let player_id: u8 = parse_msg_arg(msg_args[1], "Cannot parse player ID");
                let msg: u8 = parse_msg_arg(msg_args[2], "Cannot parse message from player");
                // TODO look up name of player
                println!("Player {} said: \"{}\"", player_id, msg);
            }

            "lose" => {
                let won: u32 = parse_msg_arg(msg_args[1], "Cannot parse amount of wins");
                let lost: u32 = parse_msg_arg(msg_args[2], "Cannot parse amount of losses");
                println!("Lost. Won {} times, lost {} times.", won, lost);
            }

            // NOP messages
            _ => {}
        };
    }
}

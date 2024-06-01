use std::fs::read_to_string;
use std::io::{BufRead, BufReader, Write};
use std::iter::Iterator;
use std::net::TcpStream;
use std::str::FromStr;

const SERVER_ADDR: &str = "151.216.74.213:4000";
const USERNAME: &str = "MASTER CONTROL PROGRAM";
const DEBUG: bool = true;

type PlayerID = u8;
type COORD = u8;

struct Game {
    username: String,
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    read_buf: String,
    // our ID - None means we don't know our ID yet
    me: Option<PlayerID>,
    // other players - ID and name
    others: Vec<Option<String>>,
    // list of fields blocked by each player. We keep this list separate from the player ID/name
    // list because we don't know whether we'll get others' name or coords first.
    blocked: Vec<Vec<(COORD, COORD)>>,
}

impl Game {
    fn new(username: &str) -> Self {
        // connect to server
        println!("Connecting to server");
        let stream = TcpStream::connect(SERVER_ADDR).expect("Cannot connect to server");
        let r = BufReader::new(stream.try_clone().expect("Cannot clone TCPStream"));

        // return game object
        Game {
            username: String::from(username),
            reader: r,
            writer: stream,
            read_buf: String::with_capacity(256),
            me: None,
            others: Vec::new(),
            blocked: Vec::new(),
        }
    }

    fn join(&mut self, pas: &str) {
        println!("Sending JOIN to join next game");
        let usr = self.username.clone();
        self.send("join", Some(&[usr.as_str(), pas]));
    }

    fn reset(&mut self) {
        self.read_buf.clear();
        self.me = None;
        self.others.clear();
        self.blocked.clear();
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

    fn add_player(&mut self, id: PlayerID, name: String) {
        if name == self.username {
            println!("Found my ID: {}", id);
            self.me = Some(id);
        } else {
            // make sure player ID is contained
            let min_len = id as usize + 1;
            while self.others.len() < min_len {
                self.others.push(None);
            }
            if self.others[id as usize] == None {
                self.others[id as usize] = Some(name);
            }
        }
    }

    fn get_player_name(&self, player_id: PlayerID) -> Option<&str> {
        match self.others.iter().nth(player_id as usize) {
            // we can't use flatten() because we're not dealing with Option<Option<T>> but
            // Option<&Option<T>> m(
            None | Some(None) => None,
            Some(Some(s)) => Some(s.as_str())
        }
    }

    fn block(&mut self, player_id: PlayerID, x: COORD, y: COORD) {
        if self.blocked.iter().nth(player_id as usize) == None {
                // make sure player ID is contained
                let min_len = player_id as usize + 1;
                if self.blocked.len() < min_len {
                    self.blocked.push(Vec::new());
                }
                self.blocked[player_id as usize] = vec![(x,y)];
        } else {
            // TODO maybe check whether that field already blocked (should be unnecessary though)
            self.blocked[player_id as usize].push((x,y));
        }
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
    let mut game = Game::new(username);

    // join next game
    game.join(password);

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
                let player_id = parse_msg_arg(msg_args[1], "Cannot parse player ID");
                let x = parse_msg_arg(msg_args[2], "Cannot parse position (x)");
                let y = parse_msg_arg(msg_args[3], "Cannot parse position (y)");
                game.block(player_id, x, y);
            }

            // log chat messages
            "chat" => {
                let id: PlayerID = parse_msg_arg(msg_args[1], "Cannot parse player ID");
                let msg = String::from(msg_args[2].trim());
                let name: String = match game.get_player_name(id) {
                    None => String::from("UNKNOWN"),
                    Some(n) => format!("\"{}\"", n),
                };
                println!("Player {} ({}) said: \"{}\"", id, name, msg);
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

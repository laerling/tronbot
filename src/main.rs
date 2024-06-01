use std::fs::read_to_string;
use std::io::{BufRead, BufReader, Write};
use std::iter::Iterator;
use std::net::TcpStream;
use std::str::FromStr;

const SERVER_ADDR: &str = "151.216.74.213:4000";
const USERNAME: &str = "MASTER CONTROL PROGRAM";
const DEBUG: bool = true;

type PlayerID = usize;
type Coord = usize;

#[derive(Clone, Copy, Debug)]
enum Direction {
    XPos,
    XNeg,
    YPos,
    YNeg,
}

#[derive(Clone)]
struct Cell {
    claimed_by: Option<PlayerID>,
}

impl Cell {
    fn new() -> Cell {
        Cell { claimed_by: None }
    }

    fn claimed(&self) -> bool {
        self.claimed_by.is_some()
    }
}

struct Game {
    username: String,
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    read_buf: String,
    // our ID - None means we don't know our ID yet
    me: Option<PlayerID>,
    // other players - ID and name
    others: Vec<Option<String>>,
    world: Vec<Vec<Cell>>,
    pos: (usize, usize),
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
            // for performance sake, assume a big world from the get-go
            world: Vec::with_capacity(50^2),
            pos: (0,0), // we assume our position will be updated soon
        }
    }

    fn join(&mut self, pas: &str) {
        println!("Sending JOIN to join next game");
        let usr = self.username.clone();
        self.send("join", Some(&[usr.as_str(), pas]));
    }

    fn reset(&mut self, x: Coord, y: Coord, me: PlayerID) {
        self.read_buf.clear();
        self.me = Some(me);
        self.others.clear();
        self.world = vec![vec![Cell::new(); x]; y];
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
            let min_len = id + 1;
            while self.others.len() < min_len {
                self.others.push(None);
            }
            if self.others[id] == None {
                self.others[id] = Some(name);
            }
        }
    }

    fn remove_player(&mut self, id: PlayerID) {

        // remove player from the list of players
        // we assume that the player ID was known (i.e. we'll not be OOB)
        self.others[id] = None;

        // free all cells that were claimed by the player
        for row in self.world.iter_mut() {
            for cell in row.iter_mut() {
                if cell.claimed_by == Some(id) {
                    cell.claimed_by = None;
                }
            }
        }
    }

    fn get_player_name(&self, player_id: PlayerID) -> Option<&str> {
        match self.others.iter().nth(player_id) {
            // we can't use flatten() because we're not dealing with Option<Option<T>> but
            // Option<&Option<T>> m(
            None | Some(None) => None,
            Some(Some(s)) => Some(s.as_str())
        }
    }

    fn occupy(&mut self, player_id: PlayerID, x: Coord, y: Coord) {
        // we assume that the field is not yet claimed by anyone
        self.world[x][y] = Cell { claimed_by: Some(player_id) };
    }

    fn say(&mut self, msg: &str) {
        self.send("chat", Some(&[msg]));
    }

    fn print_world(&self) {
        let expanse_x = self.world.len();
        println!("World (expanse_x == {}):", expanse_x);
        // we have to iterate backwards (using `rev()`) for correct orientation
        for x in (0..expanse_x).rev() {
            for y in (0..self.world[x].len()).rev() {
                let cell = &self.world[x][y];
                if cell.claimed() {
                    print!("{:02}", cell.claimed_by.unwrap());
                } else {
                    print!("--");
                }
            }
            println!()
        }
        println!("(My ID was {})", self.me.unwrap())
    }
}

fn parse_msg_arg<T: FromStr>(arg: &str, err_msg: &str) -> T {
    let arg = arg.trim();
    arg.parse()
        .unwrap_or_else(|_| panic!("{}: \"{}\"", err_msg, arg))
}

fn beam(game: &Game, dir: Direction) -> usize {
    // we assume that the world is the same size in all dimensions
    let expanse = game.world.len();
    let mut beam_len = 0;
    for offset in 0..expanse-1 {
        if !match dir {
            // `+ expanse` in *Neg cases because we cannot underflow before the modulo op
            Direction::XPos => game.world[(game.pos.0 + offset) % expanse][game.pos.1].claimed(),
            Direction::XNeg => game.world[(game.pos.0 + expanse - offset) % expanse][game.pos.1].claimed(),
            Direction::YPos => game.world[game.pos.0][(game.pos.1 + offset) % expanse].claimed(),
            Direction::YNeg => game.world[game.pos.0][(game.pos.1 + expanse - offset) % expanse].claimed(),
        } {
            beam_len += 1;
        }
    }
    beam_len
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
                let w = parse_msg_arg(msg_args[1], "Cannot parse map width");
                let h = parse_msg_arg(msg_args[2], "Cannot parse map height");
                let id = parse_msg_arg(msg_args[3], "Cannot parse ID");
                // we're arbitrarily assigning x to be the width and y to be the height
                game.reset(w, h, id);
                game.say("You shouldn't have come back, Flynn.");
            }

            // tick - make a move
            "tick" => {
                // simple strategy - beam into all four directions
                let mut best_dir = Direction::XPos;
                let mut longest_beam = 0;
                for dir in [Direction::XPos, Direction::XNeg, Direction::YPos, Direction::YNeg] {
                    let beam = beam(&game, dir);
                    if beam > longest_beam {
                        best_dir = dir;
                        longest_beam = beam;
                    }
                }
                println!("Best direction to move into is: {:?}", best_dir);

                // move into best direction
                let direction_name = match best_dir {
                    Direction::XPos => "down",
                    Direction::XNeg => "up",
                    Direction::YPos => "left",
                    Direction::YNeg => "right",
                };
                println!("Moving {}", direction_name);
                game.send("move", Some(&[direction_name]));
            }

            // register players
            "player" => {
                let id = parse_msg_arg(msg_args[1], "Cannot parse to number");
                let name = String::from(msg_args[2].trim());
                println!("Registering player {} \"{}\"", id, name);
                game.add_player(id, name);
            }

            // update blocked cells in the world
            "pos" => {

                // occupy cell
                let player_id = parse_msg_arg(msg_args[1], "Cannot parse player ID");
                let x = parse_msg_arg(msg_args[2], "Cannot parse position (x)");
                let y = parse_msg_arg(msg_args[3], "Cannot parse position (y)");
                game.occupy(player_id, x, y);

                // if the position relates to us, update our position
                if game.me == Some(player_id) {
                    if DEBUG {
                        println!("We're currently at ({},{}).", x, y);
                    }
                    game.pos = (x,y);
                }
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

            "die" => {
                let id: PlayerID = parse_msg_arg(msg_args[1], "Cannot parse player ID");
                let name: String = match game.get_player_name(id) {
                    None => String::from("UNKNOWN"),
                    Some(n) => format!("\"{}\"", n),
                };
                println!("Player {} (\"{}\") died. Removing their blocked cells.", id, name);
                game.remove_player(id);
            }

            "lose" => {
                let won: u32 = parse_msg_arg(msg_args[1], "Cannot parse amount of wins");
                let lost: u32 = parse_msg_arg(msg_args[2], "Cannot parse amount of losses");
                println!("Lost. Won {} times, lost {} times.", won, lost);
                game.print_world();
            }

            "win" => println!("THE VICTORY IS OURS!"),

            // NOP messages
            _ => {}
        };
    }
}

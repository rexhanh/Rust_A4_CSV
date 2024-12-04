use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use csv::WriterBuilder;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs::OpenOptions;
struct AppState {
    counter: Mutex<i32>,
    songs: RwLock<Vec<Song>>,
}
#[derive(Deserialize, Debug)]
struct NewSong {
    title: String,
    artist: String,
    genre: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Song {
    id: i32,
    title: String,
    artist: String,
    genre: String,
    play_count: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SearchQuery {
    title: Option<String>,
    artist: Option<String>,
    genre: Option<String>,
}

fn save_song(song: Song) {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open("songs.csv")
        .unwrap();
    let mut writer = WriterBuilder::new().has_headers(false).from_writer(file);
    writer.serialize(song).unwrap();
}

fn save_all_songs(songs: Vec<Song>) {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("songs.csv")
        .unwrap();
    let mut writer = WriterBuilder::new().has_headers(false).from_writer(file);
    writer
        .write_record(&["id", "title", "artist", "genre", "play_count"])
        .unwrap();
    for song in songs {
        writer.serialize(song).unwrap();
    }
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Welcome to the Rust-powered web server!")
}

#[get("/count")]
async fn count(data: web::Data<AppState>) -> impl Responder {
    let mut counter = data.counter.lock();
    *counter += 1;
    HttpResponse::Ok().body(format!("Visit count: {}", counter))
}

#[post("/songs/new")]
async fn new_song(song: web::Json<NewSong>, data: web::Data<AppState>) -> impl Responder {
    let mut library = data.songs.write();
    let new_song = Song {
        title: song.title.clone(),
        artist: song.artist.clone(),
        genre: song.genre.clone(),
        id: (library.len() + 1) as i32,
        play_count: 0,
    };
    library.push(new_song.clone());
    save_song(new_song.clone());
    HttpResponse::Ok().json(new_song)
}
#[get("/songs/search")]
async fn search(query: web::Query<SearchQuery>, data: web::Data<AppState>) -> impl Responder {
    let songs = data.songs.read();
    let results: Vec<Song> = songs
        .par_iter()
        .filter(|song| {
            (query.title.is_none()
                || song
                    .title
                    .to_lowercase()
                    .contains(&query.title.as_ref().unwrap().to_lowercase()))
                && (query.artist.is_none()
                    || song
                        .artist
                        .to_lowercase()
                        .contains(&query.artist.as_ref().unwrap().to_lowercase()))
                && (query.genre.is_none()
                    || song
                        .genre
                        .to_lowercase()
                        .contains(&query.genre.as_ref().unwrap().to_lowercase()))
        })
        .cloned()
        .collect();
    HttpResponse::Ok().json(results)
}

#[get("/songs/play/{id}")]
async fn play_song(path: web::Path<i32>, data: web::Data<AppState>) -> impl Responder {
    let updated_song = {
        if let Some(song) = data
            .songs
            .write()
            .par_iter_mut()
            .find_any(|song| song.id == *path)
        {
            song.play_count += 1;
            Some(song.clone())
        } else {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Song not found"
            }));
        }
    };
    save_all_songs(data.songs.write().clone());
    HttpResponse::Ok().json(updated_song)
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("The server is currently listening on localhost:8080.");
    let rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path("songs.csv");
    let mut rdr = match rdr {
        Ok(rdr) => rdr,
        Err(_) => {
            OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open("songs.csv")
                .unwrap();
            let mut wtr = WriterBuilder::new()
                .has_headers(true)
                .from_path("songs.csv")
                .unwrap();
            wtr.write_record(&["id", "title", "artist", "genre", "play_count"])
                .unwrap();
            let rdr = csv::ReaderBuilder::new()
                .has_headers(true)
                .from_path("songs.csv");
            rdr.unwrap()
        }
    };
    let songs: Vec<Song> = rdr.deserialize().map(|result| result.unwrap()).collect();
    let app_state = web::Data::new(AppState {
        counter: Mutex::new(0),
        songs: RwLock::new(songs.clone()),
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(hello)
            .service(count)
            .service(new_song)
            .service(search)
            .service(play_song)
    })
    .bind(("127.0.0.1", 8080))?
    .run();
    server.await
}

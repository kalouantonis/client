mod play_queue;

use std::error::Error;
use std::sync;

use glib;
use gstreamer as gst;
use gstreamer::prelude::*;

use self::play_queue::PlayQueue;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Track {
    id: u64,
    track: u8,
    title: String,
    artist: String,
    album: String,
    // TODO: use a URI
    file: String,
}

/// Initialize the audio subsystem used for streaming and playing audio.
///
/// Will return a string description of the internal error on failure.
pub fn init_audio_subsystem() -> Result<(), String> {
    gst::init().map_err(|e| {
        String::from(e.description())
    })
}

#[derive(PartialEq)]
enum StreamState {
    Playing,
    Paused,
    Stopped,
}

pub enum StreamEvent {
    Completed,
}

/// Represents a behaviour that can be used to stream data of any type
/// from a URI.
trait Streamable {
    /// Queue an item with the given URI in to the stream.
    fn queue(&self, uri: String);
    /// Start the stream playback
    fn start(&self);
    /// Stop the stream playback, will reset the position to the beginning.
    fn stop(&self);
    /// Pause the stream playback, maintaining the current position.
    fn pause(&self);
    /// Return the state that the stream is currently in.
    fn state(&self) -> StreamState;
    /// Return a receiver for stream events.
    fn event_listener(&self) -> sync::mpsc::Receiver<StreamEvent>;
}

impl From<gst::State> for StreamState {
    fn from(state: gst::State) -> Self {
        match state {
            gst::State::Playing => StreamState::Playing,
            gst::State::Paused  => StreamState::Paused,
            gst::State::Null
                | gst::State::Ready
                | gst::State::VoidPending => StreamState::Stopped,
            gst::State::__Unknown(e) => panic!("Stream has entered an unknown state: {}", e),
        }
    }
}

/// Internal audio streamer. Can stream audio from remote or local
/// files.
#[derive(Clone)]
struct AudioStreamer {
    // TODO: ensure that this memory isn't copied
    playbin: Box<gst::Element>,
}

impl AudioStreamer {
    fn new() -> AudioStreamer {
        AudioStreamer {
            playbin: Box::new(gst::ElementFactory::make("playbin", None).unwrap())
        }
    }

    fn change_state(&self, state: gst::State) {
        let new_state = self.playbin.set_state(state);
        assert_ne!(new_state, gst::StateChangeReturn::Failure);
    }
}

impl Streamable for AudioStreamer {
    fn queue(&self, uri: String) {
        self.playbin.set_property("uri", &glib::Value::from(&uri)).unwrap();
    }

    fn start(&self) {
        self.change_state(gst::State::Playing);
    }

    fn stop(&self) {
        self.change_state(gst::State::Null);
    }

    fn pause(&self) {
        self.change_state(gst::State::Paused);
    }

    fn state(&self) -> StreamState {
        // FIXME: Could block UI thread
        let (_, current_state, _) = self.playbin.get_state(gst::CLOCK_TIME_NONE);
        StreamState::from(current_state)
    }

    fn event_listener(&self) -> sync::mpsc::Receiver<StreamEvent> {
        let (tx, rx) = sync::mpsc::channel();
        let tx_mutex = sync::Mutex::new(tx);

        self.playbin.connect("about-to-finish", false, move |_| {
            let tx = tx_mutex.lock().unwrap();
            tx.send(StreamEvent::Completed).unwrap();
            None
        }).unwrap();

        rx
    }
}

#[derive(Clone)]
pub struct Player {
    pub is_looping: bool,
    play_queue: PlayQueue<Track>,
    // TODO; use generics
    streamer: AudioStreamer,
}

/// A command to be passed to the media player.
#[derive(Debug)]
pub enum PlayerCommand {
    /// Put a track in to the play queue
    Queue(Track),
    /// Start audio playback
    Play,
    /// Pause audio playback, keeping the track position.
    Pause,
    /// Stop audio playback, resetting track position.
    Stop,
    /// Move to the next track in the queue.
    Next,
    /// Move to the previous track in the queue.
    Previous,
    /// Kill the audio player.
    Kill,
}

pub type PlayerSender = sync::mpsc::Sender<PlayerCommand>;

impl Player {
    /// Create a new audio player with no queued tracks.
    pub fn new() -> Player {
        Player {
            is_looping: false,
            play_queue: PlayQueue::new(),
            streamer: AudioStreamer::new(),
        }
    }

    /// Create an event listener that receives messages over
    /// a channel and performs the relevant command.
    pub fn event_listener(mut self) -> PlayerSender {
        let (tx, rx) = sync::mpsc::channel();

        // Add runner in to glib event loop
        glib::idle_add(move || {
            let mut should_continue = true;

            if let Ok(cmd) = rx.try_recv() {
                trace!("Running player command: {:?}", cmd);
                // TODO: handle errors
                use self::PlayerCommand::*;
                match cmd {
                    Queue(ref track) => self.queue(track),
                    Play  => self.play(),
                    Pause => self.pause(),
                    Stop  => self.stop(),
                    Next  => self.next_track(),
                    Previous => self.previous_track(),
                    Kill => should_continue = false,
                }
            }

            glib::Continue(should_continue)
        });

        tx
    }

    /// Returns `true` if the player is currently streaming audio.
    ///
    /// WARNING: May block the UI thread.
    fn is_playing(&self) -> bool {
        self.streamer.state() == StreamState::Playing
    }

    /// Returns the current track in the queue. May or may not
    /// be currently playing.
    fn current_track(&self) -> Option<&Track> {
        self.play_queue.current()
    }

    /// Add a track to the end of the play queue.
    fn queue(&mut self, track: &Track) {
        self.play_queue.append(track);
    }

    /// Begin playback of the current track.
    ///
    /// Will panic if the player is already playing.
    fn play(&self) {
        assert!(self.streamer.state() != StreamState::Playing);

        if let Some(track) = self.current_track() {
            // TODO: investigate clone
            self.streamer.queue(track.file.clone());
            self.streamer.start();
        }
    }

    /// Pause playback of the current track. Will do nothing if
    /// there is no current track.
    ///
    /// Pausing keeps the previous location of the audio stream.
    ///
    /// Will panic if the player is already paused.
    fn pause(&self) {
        assert!(self.streamer.state() != StreamState::Paused);
        self.streamer.pause();
    }

    /// Stop playback of the current track.
    ///
    /// Will do nothing if player is already stopped.
    fn stop(&self) {
        if self.is_playing() {
            self.streamer.stop()
        }
    }

    /// Stop the current track and play the next one in the queue.
    ///
    /// If there are no more items, the player will stop, unless `is_looping`
    /// is set to true, in which case it will start again from the beginning.
    fn next_track(&mut self) {
        self.stop();

        if self.play_queue.next().is_none() {
            if self.is_looping {
                self.play_queue.reset();
            } else {
                return;
            }
        }

        self.play();
    }

    /// Stop the current track and play the previous one in the queue.
    ///
    /// If there are no more previous items the track will be started
    /// from the beginning.
    fn previous_track(&mut self) {
        self.stop();
        self.play_queue.previous();
        self.play();
    }
}

# MiniRAW NG - simple print job listener written in Rust

This application will listen on port 9100 for incoming connections and save the data into files in the same directory where exe file is located.
Received files can be discarded by toggling the "Discard received files" menu item in the windows system menu.

The main purpose of this simple application is to try a low-level GUI programming in Rust.

It is inspired by the old "miniraw" utility written by Rocco Lapadula. Compared to the original version mine is:

* Open source
* Written in modern language
* Doesn't suffer from occasional issues with premature socket shutdown

This is a GUI utility currently working on Windows.
Binary releases can be downloaded from Releases section.

## License

Licensed under MIT or Apache license ([LICENSE-MIT](https://opensource.org/licenses/MIT) or [LICENSE-APACHE](https://opensource.org/licenses/Apache-2.0))

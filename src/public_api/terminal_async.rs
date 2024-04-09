/*
 *   Copyright (c) 2024 R3BL LLC
 *   All rights reserved.
 *
 *   Licensed under the Apache License, Version 2.0 (the "License");
 *   you may not use this file except in compliance with the License.
 *   You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 *   Unless required by applicable law or agreed to in writing, software
 *   distributed under the License is distributed on an "AS IS" BASIS,
 *   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *   See the License for the specific language governing permissions and
 *   limitations under the License.
 */

use crate::{FuturesMutex, Readline, ReadlineEvent, SharedWriter};
use crossterm::style::Stylize;
use futures_util::FutureExt;
use miette::IntoDiagnostic;
use r3bl_tuify::{
    is_fully_uninteractive_terminal, is_stdin_piped, is_stdout_piped, StdinIsPipedResult,
    StdoutIsPipedResult, TTYResult,
};
use std::{
    io::{stdout, Write},
    sync::Arc,
};

pub struct TerminalAsync {
    pub readline: Readline,
    pub shared_writer: SharedWriter,
}

// 01: add tests

impl TerminalAsync {
    /// Create a new instance of [TerminalAsync]. Example of `prompt` is `"> "`.
    ///
    /// ### Returns
    /// 1. If the terminal is not fully interactive then it will return [None], and won't
    ///    create the [Readline]. This is when the terminal is not considered fully
    ///    interactive:
    ///    - `stdout` is piped, eg: `echo "foo" | cargo run --example spinner`.
    ///    - or all three `stdin`, `stdout`, `stderr` are not `is_tty`, eg when running in
    ///      `cargo test`.
    /// 2. Otherwise, it will return a [TerminalAsync] instance.
    /// 3. In case there are any issues putting the terminal into raw mode, or getting the
    ///    terminal size, it will return an error.
    ///
    /// More info on terminal piping:
    /// - <https://unix.stackexchange.com/questions/597083/how-does-piping-affect-stdin>
    pub async fn try_new(prompt: &str) -> miette::Result<Option<TerminalAsync>> {
        if let StdinIsPipedResult::StdinIsPiped = is_stdin_piped() {
            return Ok(None);
        }
        if let StdoutIsPipedResult::StdoutIsPiped = is_stdout_piped() {
            return Ok(None);
        }
        if let TTYResult::IsNotInteractive = is_fully_uninteractive_terminal() {
            return Ok(None);
        }

        let raw_term = Arc::new(FuturesMutex::new(stdout()));
        let (readline, stdout) = Readline::new(prompt.to_owned(), raw_term)
            .await
            .into_diagnostic()?;
        Ok(Some(TerminalAsync {
            readline,
            shared_writer: stdout,
        }))
    }

    pub fn clone_flush_signal_sender(
        &self,
    ) -> tokio::sync::mpsc::Sender<crate::ReadlineFlushSignal> {
        self.readline.flush_signal_sender.clone()
    }

    pub fn clone_shared_writer(&self) -> SharedWriter {
        self.shared_writer.clone()
    }

    /// Replacement for [std::io::Stdin::read_line()] (this is async and non blocking).
    pub async fn get_readline_event(&mut self) -> miette::Result<ReadlineEvent> {
        self.readline.readline().fuse().await.into_diagnostic()
    }

    /// Don't change the `content`. Print it as is. This works concurrently and is async
    /// and non blocking. And it is compatible w/ the
    /// [get_readline_event](TerminalAsync::get_readline_event) method.
    pub async fn println<T>(&mut self, content: T)
    where
        T: std::fmt::Display,
    {
        let _ = writeln!(self.shared_writer, "{}", content);
    }

    /// Prefix the `content` with a color and special characters, then print it.
    pub async fn println_prefixed<T>(&mut self, content: T)
    where
        T: std::fmt::Display,
    {
        let _ = writeln!(
            self.shared_writer,
            "{} {}",
            " > ".red().bold().on_dark_grey(),
            content
        );
    }

    /// Simply flush the buffer. If there's a newline in the buffer, it will be printed.
    /// Otherwise it won't.
    pub async fn flush(&mut self) {
        let _ = self.readline.flush().await;
    }

    pub async fn suspend(&mut self) {
        self.readline.suspend().await;
    }

    pub async fn resume(&mut self) {
        self.readline.resume().await;
        // 00: clean this up
        // self.flush().await;
    }

    /// Close the underlying [Readline] instance. This will terminate all the tasks that
    /// are managing [SharedWriter] tasks. This is useful when you want to exit the CLI
    /// event loop, typically when the user requests it.
    pub async fn close(&mut self) {
        self.readline.close();
    }
}

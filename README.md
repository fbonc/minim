<p align="center">
  <img width="350" alt="banner" src="https://github.com/user-attachments/assets/e714ec64-e4dc-4d10-845f-17a037229b07" />
</p>

```Previously "Little Owl"```

An app for asking questions about what you are reading or viewing, right when a question comes to mind.

A linear chat thread is fundamentally a flawed medium for atomic questions.  is being built to keep each answer connected to its source, so useful explanations do not disappear into a chat thread.

## Why

Copying something into a chat loses the surrounding context. Switching apps interrupts the question, and small, unrelated answers get buried in a long conversation. This is especially awkward when reading a paper or another document you cannot edit: a term needs a quick explanation, but asking about it takes you away from the page.

Ruru aims to eliminate the friction from having a question to asking it, and then organize the answers by where those questions came from.

## How it works

1. Highlight text and press the global hotkey. Mininm captures the selection, nearby context, and available source details such as the app, window, or document. You can also select a screen region as the subject of a question.
2. Ask in the small overlay without leaving the source. The answer appears there as it streams.
3. The planned history will save each answer as a separate Markdown note with its provenance, so questions from the same document can be found together.

For example, highlighting an unfamiliar term in a PDF should give the model the term, surrounding text when available, and the document it came from. If the surrounding text cannot be read (either via the accessibility API or a synthetic clipboard), a screenshot can provide context. A selected screen region can serve as an image subject when the question is about a figure or other visual content.

- **Capture independent pieces of context.** The selected text, surrounding context, and source identity can succeed or fail separately. A missing document path should not discard a useful selection.
- **Prefer text, fall back where needed.** macOS Accessibility reads selected text and nearby content; the clipboard can recover a selection when Accessibility cannot. A screenshot can supply missing context, while a user-selected region supplies an image subject.
- **Keep the interface responsive.** Capture runs outside the UI thread. The core tracks each lookup so a new question cannot display chunks from an older answer.

## Architecture

This is a Rust workspace with an Iced desktop UI. `-hotkey` starts a lookup; `-core` coordinates capture and the answer request; `-capture` owns the macOS Accessibility, clipboard, screenshot, and provenance code; and `-provider` defines the streaming model interface. `-app` hosts the `-ui` overlay, while `-types` holds the data shared between crates.

The capture and overlay flow is implemented. Model integration and source-linked answer history are the next pieces of the intended app.

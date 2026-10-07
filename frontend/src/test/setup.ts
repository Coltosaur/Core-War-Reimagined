import '@testing-library/jest-dom/vitest';

// jsdom (29) defines HTMLDialogElement but neither showModal() nor close().
// This stand-in only toggles `open` and fires `close` the way the browser
// does. It deliberately fakes none of the modal behaviour (top layer, inert
// background, Escape, focus restoration), so unit tests can't pass on the
// strength of the polyfill: those are verified in a real browser instead.
if (typeof HTMLDialogElement !== 'undefined' && !HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    if (!this.open) return;
    this.open = false;
    this.dispatchEvent(new Event('close'));
  };
}

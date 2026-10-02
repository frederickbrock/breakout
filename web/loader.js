// trunk initializer for the Steelbreak loading splash (see index.html).
//
// trunk calls these hooks while it downloads and starts the wasm:
// onStart → onProgress({ current, total })* → onComplete → onSuccess | onFailure.
// The splash stays up after the wasm starts; the game dispatches
// `steelbreak-ready` on window once it has drawn its main menu
// (src/web_splash.rs), and only then does the splash fade out.
// Any failure before that shows "Couldn't start the game: <reason>" + Reload.

const MB = 1024 * 1024;
// winit escapes its event loop by throwing this on purpose; not a failure.
const WINIT_CONTROL_FLOW = "Using exceptions for control flow";

export default function steelbreakLoader() {
  const splash = document.getElementById("splash");
  const bar = document.getElementById("splash-bar");
  const fill = document.getElementById("splash-fill");
  const amount = document.getElementById("splash-amount");
  const status = document.getElementById("splash-status");
  const errorBox = document.getElementById("splash-error");
  const errorText = document.getElementById("splash-error-text");
  let finished = false;

  const setStatus = (text) => {
    status.textContent = text;
  };

  const fail = (reason) => {
    if (finished) return;
    finished = true;
    splash.classList.add("failed");
    bar.classList.remove("indeterminate");
    errorText.textContent = `Couldn't start the game: ${reason}`;
    errorBox.hidden = false;
    document.getElementById("splash-reload").focus();
  };

  const reasonOf = (error) => {
    if (!error) return "unknown error";
    if (typeof error === "string") return error;
    return error.message || String(error);
  };

  // Errors while the splash is up (e.g. a panic during start-up) mean the
  // game won't appear: say so instead of leaving the splash spinning.
  const onError = (event) => {
    const reason = reasonOf(event.error || event.reason || event.message);
    if (reason.includes(WINIT_CONTROL_FLOW)) return;
    fail(reason);
  };
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onError);

  window.addEventListener(
    "steelbreak-ready",
    () => {
      if (finished) return;
      finished = true;
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onError);
      splash.classList.add("done");
      // Remove after the fade (and on a timer, in case transitions are off).
      const remove = () => splash.remove();
      splash.addEventListener("transitionend", remove, { once: true });
      setTimeout(remove, 1000);
    },
    { once: true },
  );

  document.getElementById("splash-reload").addEventListener("click", () => {
    window.location.reload();
  });

  return {
    onStart: () => {
      setStatus("Downloading…");
      const canvas = document.createElement("canvas");
      if (!canvas.getContext("webgl2")) {
        fail("this browser doesn't support WebGL2");
      }
    },
    onProgress: ({ current, total }) => {
      if (finished) return;
      const loaded = (current / MB).toFixed(1);
      // The wasm is served gzip-encoded: `total` can be missing, or be the
      // compressed size while `current` counts decompressed bytes. Only show
      // a fraction when it makes sense; otherwise an indeterminate bar.
      if (total && current <= total) {
        bar.classList.remove("indeterminate");
        fill.style.width = `${((current / total) * 100).toFixed(1)}%`;
        amount.textContent = `${loaded} / ${(total / MB).toFixed(1)} MB`;
      } else {
        bar.classList.add("indeterminate");
        fill.style.width = "";
        amount.textContent = `${loaded} MB`;
      }
    },
    onComplete: () => {
      if (finished) return;
      bar.classList.remove("indeterminate");
      fill.style.width = "100%";
      setStatus("Starting…");
    },
    onSuccess: () => {},
    onFailure: (error) => fail(reasonOf(error)),
  };
}

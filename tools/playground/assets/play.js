/**
 * Editor page of the playground: sends the screen body to /run and shows the rendered page,
 * served from the preview origin, in the frame.
 */
const code = document.getElementById("code");
const run = document.getElementById("run");
const status = document.getElementById("status");
const log = document.getElementById("log");
const frame = document.getElementById("preview");

/** Runs the code in the box and shows the result. */
async function play() {
  run.disabled = true;
  status.textContent = "Compiling...";
  log.hidden = true;
  try {
    const response = await fetch("/run", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ body: code.value }),
    });
    const answer = await response.json();
    if (answer.preview) {
      frame.src = answer.preview;
      status.textContent = answer.cached
        ? "Shown from the cache."
        : `Done in ${answer.total_ms} ms (${answer.stats}).`;
    } else {
      status.textContent = answer.error || `Failed with ${response.status}.`;
      if (answer.log) {
        log.textContent = answer.log;
        log.hidden = false;
      }
    }
  } catch (e) {
    status.textContent = "The playground did not answer.";
  } finally {
    run.disabled = false;
  }
}

run.addEventListener("click", play);
code.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    play();
  }
});
play();

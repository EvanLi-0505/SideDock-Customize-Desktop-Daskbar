// Icons come from the backend (`sdicon://`). One that cannot be extracted at that moment
// (an app being updated, a window whose file is still locked) answers 404 instead of an
// empty picture; this asks for it again a few times, a little later each time.

const ICON_ORIGIN = "http://sdicon.localhost/";
const DELAYS = [700, 2000, 6000];

/** Installs the retry for every icon of this page (once, at startup). */
export function installIconRetry(): void {
  window.addEventListener(
    "error",
    (e) => {
      const img = e.target;
      if (!(img instanceof HTMLImageElement) || !img.src.startsWith(ICON_ORIGIN)) return;
      const url = new URL(img.src);
      const attempt = Number(url.searchParams.get("retry") ?? "0");
      const delay = DELAYS[attempt];
      if (delay === undefined) return;
      setTimeout(() => {
        // the element may have been given another icon meanwhile
        const current = new URL(img.src);
        current.searchParams.delete("retry");
        url.searchParams.delete("retry");
        if (current.toString() !== url.toString()) return;
        url.searchParams.set("retry", String(attempt + 1));
        img.src = url.toString();
      }, delay);
    },
    // image errors do not bubble: listen while they are captured
    true,
  );
}

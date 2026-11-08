/** The webview origin may not count as secure, so the async clipboard can be missing or refuse. */
export async function copyText(text: string): Promise<boolean> {
  const written = navigator.clipboard ? await navigator.clipboard.writeText(text).then(() => true, () => false) : false;
  if (written) return true;
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.cssText = 'position:fixed;top:0;left:0;opacity:0;pointer-events:none';
  // Outside an open modal <dialog> everything is inert, so the textarea must live inside it.
  (document.querySelector('dialog[open]') ?? document.body).append(area);
  area.select();
  let copied = false;
  try {
    copied = document.execCommand('copy');
  } catch {
    copied = false;
  }
  area.remove();
  return copied;
}

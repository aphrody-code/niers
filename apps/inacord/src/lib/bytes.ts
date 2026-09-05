export function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function bytesToB64(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

export function humanSize(n: number): string {
  if (n < 1024) return `${n} o`;
  const units = ["Ko", "Mo", "Go"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 2 : 1)} ${units[i]}`;
}

/** Une ligne hexdump façon `xxd` : offset + 16 octets hex + colonne ASCII. */
export function hexLines(bytes: Uint8Array, maxLines = 512): string[] {
  const lines: string[] = [];
  const n = Math.min(bytes.length, maxLines * 16);
  for (let off = 0; off < n; off += 16) {
    const chunk = bytes.subarray(off, off + 16);
    let hex = "";
    let ascii = "";
    for (let j = 0; j < 16; j++) {
      if (j < chunk.length) {
        const b = chunk[j];
        hex += b.toString(16).padStart(2, "0") + " ";
        ascii += b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : ".";
      } else {
        hex += "   ";
      }
    }
    lines.push(`${off.toString(16).padStart(8, "0")}  ${hex} ${ascii}`);
  }
  return lines;
}

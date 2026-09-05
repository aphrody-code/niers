import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { Editeur } from "../Editeur";
import type { Catalogue } from "../types";

// Capture uniquement dans ce harnais : le téléchargement réel reste exécuté.
const creerURL = URL.createObjectURL.bind(URL);
URL.createObjectURL = (blob: Blob | MediaSource) => {
	if (blob instanceof Blob) void blob.arrayBuffer().then(buffer => {
		const vue = new DataView(buffer);
		let preuve: unknown;
		if (buffer.byteLength >= 20 && vue.getUint32(0, true) === 0x46546c67) {
			const gltf = JSON.parse(new TextDecoder().decode(new Uint8Array(buffer, 20, vue.getUint32(12, true))));
			preuve = { format: "GLB", octets: buffer.byteLength, racines: gltf.scenes[gltf.scene ?? 0].nodes.map((i: number) => gltf.nodes[i]), mailles: gltf.meshes?.length };
		} else if (blob.type === "application/json") preuve = JSON.parse(new TextDecoder().decode(buffer));
		else return;
		document.getElementById("preuves")!.textContent = JSON.stringify(preuve, null, 2);
	});
	return creerURL(blob);
};

const catalogue = await (await fetch("/avatar/catalog.json")).json() as Catalogue;
let terminerExport: (() => void) | undefined;
function Harnais() {
	const [cle, setCle] = useState(0);
	const [visible, setVisible] = useState(true);
	async function injecter(fichiers: File[]) {
		const input = document.querySelector<HTMLInputElement>('input[aria-label="Importer des fichiers avatar"]');
		if (!input) return;
		const transfert = new DataTransfer(); fichiers.forEach(f => transfert.items.add(f)); input.files = transfert.files;
		input.dispatchEvent(new Event("change", { bubbles: true }));
	}
	return <><button onClick={() => setCle(c => c + 1)}>Recréer l’atelier</button>
		<button onClick={async () => {
			const c = document.createElement("canvas"); c.width = 96; c.height = 128;
			const ctx = c.getContext("2d")!;
			for (let i = 0; i < 12; i++) { ctx.fillStyle = `hsl(${i * 30} 80% 50%)`; ctx.fillRect(i % 3 * 32, Math.floor(i / 3) * 32, 32, 32); }
			const blob = await new Promise<Blob>(resolve => c.toBlob(b => resolve(b!), "image/png"));
			await injecter([new File([blob], "chara-test.png", { type: "image/png" })]);
		}}>Tester import PNG</button>
		<button onClick={async () => injecter([new File([await (await fetch("/fixture.glb")).blob()], "byron.glb")])}>Tester import GLB</button>
		<button onClick={async () => injecter([new File([await (await fetch("/fixture.glb.gz")).blob()], "byron.glb.gz")])}>Tester import gzip</button>
		<button onClick={() => injecter([new File(["invalide"], "erreur.glb")])}>Tester import invalide</button>
		<button onClick={() => {
			const mv = document.querySelector("model-viewer") as (HTMLElement & { exportScene(): Promise<Blob> }) | null;
			if (!mv) return;
			const original = mv.exportScene.bind(mv);
			mv.exportScene = async () => {
				mv.exportScene = original;
				await new Promise<void>(resolve => { terminerExport = resolve; });
				return original();
			};
		}}>Différer le prochain export</button>
		<button onClick={() => { terminerExport?.(); terminerExport = undefined; }}>Terminer l’export différé</button>
		<button onClick={() => document.querySelector("model-viewer")?.dispatchEvent(new Event("error"))}>Simuler une erreur du viewer</button>
		<button onClick={() => setVisible(v => !v)}>Monter / démonter</button>
		{visible && <Editeur key={cle} catalogue={catalogue} cdn={location.origin} />}</>;
}
createRoot(document.getElementById("app")!).render(<StrictMode><Harnais /></StrictMode>);

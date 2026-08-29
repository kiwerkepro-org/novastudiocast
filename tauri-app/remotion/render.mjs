#!/usr/bin/env node
// NovaStudioCast, Schritt 4 Renderskript.
//
// Wird vom Rust Controller (pipeline/render.rs) als Sidecar Argument an
// den mitgelieferten Node Laufzeit Sidecar übergeben, siehe
// docs/ARCHITEKTUR.md, Abschnitt "Remotion Anbindung, Architekturentscheidung".
//
// Aufgabe dieses Skripts: das vom Rust Controller geschriebene Gesamt
// Manifest (docs/JSON_SCHEMA.md, Ebene 3) sowie alle einzelnen
// Schnittlisten (Ebene 2) von der Festplatte lesen, daraus EINE flache,
// bereits vollständig aufgelöste Liste aus Wiedergabe Abschnitten bauen
// (siehe src/types.ts, PlaybackSegment), und diese Liste als inputProps an
// die Remotion Komposition "novastudiocast-timeline" übergeben. Die
// Komposition selbst kennt weder das Rust Manifest noch die einzelnen
// Schnittlisten, siehe Kopfkommentar in src/types.ts für die Begründung.
//
// Aufruf: node render.mjs <manifestPfad> <ausgabePfad.mp4>
//
// Gibt bei Erfolg genau eine Zeile "NOVASTUDIOCAST_RENDER_OK <ausgabePfad>"
// auf stdout aus, bei Fehlern eine Meldung auf stderr und Exitcode 1, damit
// run_sidecar (sidecar.rs) das zuverlässig auswerten kann.

import { bundle } from "@remotion/bundler";
import { renderMedia, selectComposition } from "@remotion/renderer";
import { readFile } from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import serveHandler from "serve-handler";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Bisher noch kein Feld im Rust Manifest für die Zielauflösung, siehe TODO
// in ARCHITEKTUR.md. Bis dahin fest auf Full HD, gängigster Fall für
// Bildschirmaufnahmen und Talking Head Videos.
const DEFAULT_WIDTH = 1920;
const DEFAULT_HEIGHT = 1080;
const DEFAULT_FPS = 30;

async function readJson(filePath) {
	const raw = await readFile(filePath, "utf-8");
	return JSON.parse(raw);
}

// Remotions Kompositionen laufen im Browser Kontext (bzw. dem headless
// Chromium der Renderer) und haben deshalb bewusst KEINEN Zugriff auf
// beliebige absolute Dateisystempfade, siehe die offizielle Remotion
// Dokumentation zu "Files with absolute paths"
// (https://www.remotion.dev/docs/miscellaneous/absolute-paths). Deshalb
// startet dieses Skript einen eigenen, minimalen lokalen HTTP Server, der
// genau den Arbeitsordner des Batches (denselben Ordner, in dem auch das
// Manifest selbst liegt) freigibt, und wandelt jeden absoluten Dateipfad
// aus dem Manifest in eine passende http://localhost URL relativ zu diesem
// Ordner um. Läuft nur lokal auf einem zufälligen freien Port, ausschließlich
// für die Dauer dieses einen Rendervorgangs.
function startLocalStaticServer(rootDir) {
	return new Promise((resolve, reject) => {
		const server = http.createServer((request, response) => {
			serveHandler(request, response, { public: rootDir });
		});
		server.on("error", reject);
		server.listen(0, "127.0.0.1", () => {
			const { port } = server.address();
			resolve({ server, baseUrl: `http://127.0.0.1:${port}` });
		});
	});
}

function toServedUrl(absolutePath, rootDir, baseUrl) {
	const relative = path.relative(rootDir, absolutePath);
	if (relative.startsWith("..")) {
		throw new Error(
			`Datei liegt außerhalb des Arbeitsordners und kann nicht an Remotion übergeben werden: ${absolutePath}`,
		);
	}
	const urlPath = relative.split(path.sep).map(encodeURIComponent).join("/");
	return `${baseUrl}/${urlPath}`;
}

async function buildPlaybackSegments(manifest, manifestDir, baseUrl) {
	const sortedTimeline = [...manifest.timeline].sort((a, b) => a.order - b.order);
	const segments = [];

	for (const entry of sortedTimeline) {
		const cutListPath = path.isAbsolute(entry.cutListPath)
			? entry.cutListPath
			: path.join(manifestDir, entry.cutListPath);
		const cutList = await readJson(cutListPath);

		const videoUrl = toServedUrl(
			path.resolve(entry.processedVideoPath),
			manifestDir,
			baseUrl,
		);

		for (const keep of cutList.keepSegments) {
			segments.push({
				clipId: entry.clipId,
				order: entry.order,
				src: videoUrl,
				startSeconds: keep.startSeconds,
				endSeconds: keep.endSeconds,
			});
		}
	}

	return segments;
}

async function main() {
	const [, , manifestPathArg, outputPathArg] = process.argv;
	if (!manifestPathArg || !outputPathArg) {
		console.error(
			"Aufruf: node render.mjs <manifestPfad> <ausgabePfad.mp4>",
		);
		process.exit(1);
	}

	const manifestPath = path.resolve(manifestPathArg);
	const outputPath = path.resolve(outputPathArg);
	const manifestDir = path.dirname(manifestPath);

	console.log(`Lese Manifest: ${manifestPath}`);
	const manifest = await readJson(manifestPath);

	if (!manifest.timeline || manifest.timeline.length === 0) {
		console.error("Manifest enthält keine Zeitleisten Einträge, Abbruch.");
		process.exit(1);
	}

	console.log("Starte lokalen Dateiserver für die Quelldateien…");
	const { server, baseUrl } = await startLocalStaticServer(manifestDir);

	try {
		const segments = await buildPlaybackSegments(manifest, manifestDir, baseUrl);
		console.log(
			`${segments.length} Wiedergabe Abschnitte aus ${manifest.timeline.length} Clips aufgelöst.`,
		);

		const inputProps = {
			segments,
			transitionSeconds: manifest.transitionSeconds ?? 0.15,
			fps: DEFAULT_FPS,
			width: DEFAULT_WIDTH,
			height: DEFAULT_HEIGHT,
		};

		console.log("Bündle die Remotion Komposition…");
		const serveUrl = await bundle({
			entryPoint: path.join(__dirname, "src", "index.ts"),
		});

		console.log("Ermittle Komposition und errechnete Metadaten…");
		const composition = await selectComposition({
			serveUrl,
			id: "novastudiocast-timeline",
			inputProps,
		});

		console.log(
			`Starte Rendering, ${composition.durationInFrames} Frames bei ${composition.fps} fps…`,
		);
		await renderMedia({
			composition,
			serveUrl,
			codec: "h264",
			outputLocation: outputPath,
			inputProps,
			onProgress: ({ progress }) => {
				console.log(`Fortschritt: ${Math.round(progress * 100)}%`);
			},
		});

		console.log(`NOVASTUDIOCAST_RENDER_OK ${outputPath}`);
	} finally {
		server.close();
	}
}

main().catch((error) => {
	console.error(`Fehler beim Rendern: ${error.stack ?? error.message ?? error}`);
	process.exit(1);
});

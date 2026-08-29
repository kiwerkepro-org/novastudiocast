// Gemeinsame Typen für die NovaStudioCast Remotion Komposition.
//
// Wichtig: dies ist NICHT dasselbe Format wie das Rust Gesamt Manifest aus
// docs/JSON_SCHEMA.md (Ebene 3). Das Rust Manifest referenziert pro
// Zeitleisten Eintrag nur einen Pfad zu einer eigenen Schnittliste
// (Ebene 2), Remotions Komposition läuft aber im Browser Kontext ohne
// Dateisystemzugriff. Deshalb liest render.mjs auf der Node Seite sowohl
// das Gesamt Manifest als auch alle einzelnen Schnittlisten von der
// Festplatte und baut daraus schon vorab genau eine flache, fertig
// aufgelöste Liste aus Wiedergabe Abschnitten (PlaybackSegment). Nur diese
// flache Liste wird der Komposition als inputProps übergeben.

export type PlaybackSegment = {
	clipId: string;
	order: number;
	/** Absoluter Pfad zur veredelten Videodatei des Clips (processedVideoPath aus dem Manifest). */
	src: string;
	/** Beginn des zu behaltenden Abschnitts innerhalb der Quelldatei, in Sekunden. */
	startSeconds: number;
	/** Ende des zu behaltenden Abschnitts innerhalb der Quelldatei, in Sekunden. */
	endSeconds: number;
};

export type TimelineProps = {
	/** Bereits vollständig aufgelöste, in Abspielreihenfolge sortierte Liste aller Behalten Abschnitte über alle Clips hinweg. */
	segments: PlaybackSegment[];
	/** Weiche Überblendung zwischen zwei aufeinanderfolgenden Abschnitten, in Sekunden. */
	transitionSeconds: number;
	/** Ziel Bildrate der Komposition. */
	fps: number;
	width: number;
	height: number;
};

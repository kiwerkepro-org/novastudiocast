// Registriert alle Remotion Kompositionen des Projekts. Wird sowohl beim
// Bundeln (siehe render.mjs, bundle()) als auch potenziell in der
// Remotion Studio Vorschau während der Entwicklung geladen.
//
// Es gibt genau eine Komposition, "novastudiocast-timeline", das fertige
// Gesamtvideo aus Schritt 4. Die Bilddauer und die Video Eigenschaften
// (fps, Breite, Höhe) stehen erst zur Renderzeit fest, abhängig vom
// übergebenen Manifest, deshalb wird calculateMetadata verwendet statt
// fester Werte.

import React from "react";
import { Composition } from "remotion";
import { NovaStudioCastTimeline, calculateTimelineMetadata } from "./Timeline";
import type { TimelineProps } from "./types";

const DEFAULT_PROPS: TimelineProps = {
	segments: [],
	transitionSeconds: 0.15,
	fps: 30,
	width: 1920,
	height: 1080,
};

export const RemotionRoot: React.FC = () => {
	return (
		<Composition
			id="novastudiocast-timeline"
			component={NovaStudioCastTimeline}
			durationInFrames={30}
			fps={30}
			width={1920}
			height={1080}
			defaultProps={DEFAULT_PROPS}
			calculateMetadata={calculateTimelineMetadata}
		/>
	);
};

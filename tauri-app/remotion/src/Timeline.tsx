// Die eigentliche Komposition für das fertige NovaStudioCast Gesamtvideo.
//
// Reiht alle in `segments` übergebenen Behalten Abschnitte in der
// vorgegebenen Reihenfolge hintereinander, mit einer weichen Überblendung
// (`transitionSeconds`) an jedem Übergang, siehe
// docs/JSON_SCHEMA.md ("Ebene 3", Feld `transitionSeconds`). `segments`
// ist bereits vollständig aufgelöst (siehe render.mjs), die Komposition
// selbst liest keine JSON Dateien und kennt weder das Rust Manifest noch
// die einzelnen Schnittlisten.
//
// Jeder Abschnitt wird über OffthreadVideo mit trimBefore/trimAfter aus der
// jeweiligen Quelldatei (`src`, die veredelte Videodatei des Clips)
// herausgeschnitten. OffthreadVideo extrahiert Bilder zeitbasiert über
// ffmpeg, unabhängig von der ursprünglichen Bildrate der Quelldatei, ein
// Wechsel der Ziel Bildrate ist deshalb unkritisch.

import React from "react";
import {
	AbsoluteFill,
	OffthreadVideo,
	type CalculateMetadataFunction,
} from "remotion";
import {
	TransitionSeries,
	linearTiming,
} from "@remotion/transitions";
import { fade } from "@remotion/transitions/fade";
import type { TimelineProps } from "./types";

function secondsToFrames(seconds: number, fps: number): number {
	return Math.max(1, Math.round(seconds * fps));
}

/**
 * Errechnet zur Renderzeit die tatsächliche Gesamtdauer, Bildrate und
 * Auflösung der Komposition aus den übergebenen Props. Nötig, weil die
 * Anzahl und Länge der Abschnitte erst durch das Rust Manifest bekannt ist,
 * nicht vorher fest im Code steht.
 */
export const calculateTimelineMetadata: CalculateMetadataFunction<
	TimelineProps
> = ({ props }) => {
	const { segments, transitionSeconds, fps, width, height } = props;

	if (segments.length === 0) {
		return { durationInFrames: 1, fps, width, height };
	}

	const transitionFrames = secondsToFrames(transitionSeconds, fps);
	const segmentFrames = segments.map((segment) =>
		secondsToFrames(segment.endSeconds - segment.startSeconds, fps),
	);

	const totalSegmentFrames = segmentFrames.reduce((sum, f) => sum + f, 0);
	const overlapCount = Math.max(0, segments.length - 1);
	// TransitionSeries überlappt an jedem Übergang um genau die
	// Übergangsdauer, die Gesamtdauer ist deshalb die Summe aller
	// Abschnitte minus die Summe aller Überlappungen.
	const durationInFrames = Math.max(
		1,
		totalSegmentFrames - overlapCount * transitionFrames,
	);

	return { durationInFrames, fps, width, height };
};

export const NovaStudioCastTimeline: React.FC<TimelineProps> = ({
	segments,
	transitionSeconds,
	fps,
}) => {
	if (segments.length === 0) {
		return (
			<AbsoluteFill style={{ backgroundColor: "black" }} />
		);
	}

	const transitionFrames = secondsToFrames(transitionSeconds, fps);

	return (
		<TransitionSeries>
			{segments.map((segment, index) => {
				const durationInFrames = secondsToFrames(
					segment.endSeconds - segment.startSeconds,
					fps,
				);
				const trimBeforeFrames = secondsToFrames(segment.startSeconds, fps);

				return (
					<React.Fragment key={`${segment.clipId}-${segment.order}-${index}`}>
						<TransitionSeries.Sequence durationInFrames={durationInFrames}>
							<AbsoluteFill style={{ backgroundColor: "black" }}>
								<OffthreadVideo
									src={segment.src}
									trimBefore={trimBeforeFrames}
									trimAfter={trimBeforeFrames + durationInFrames}
									pauseWhenBuffering
								/>
							</AbsoluteFill>
						</TransitionSeries.Sequence>
						{index < segments.length - 1 ? (
							<TransitionSeries.Transition
								presentation={fade()}
								timing={linearTiming({ durationInFrames: transitionFrames })}
							/>
						) : null}
					</React.Fragment>
				);
			})}
		</TransitionSeries>
	);
};

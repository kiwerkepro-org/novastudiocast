// Einstiegspunkt für Remotions eigenen Bundler (siehe render.mjs, bundle()).
// Registriert lediglich RemotionRoot mit allen darin definierten
// Kompositionen, enthält selbst keine fachliche Logik.

import { registerRoot } from "remotion";
import { RemotionRoot } from "./Root";

registerRoot(RemotionRoot);

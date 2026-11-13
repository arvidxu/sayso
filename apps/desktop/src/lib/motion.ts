import type { Transition } from "motion/react";
import { BACKDROP_FADE_MS } from "@/lib/backdrop";

export const BACKDROP_FADE: Transition = { duration: BACKDROP_FADE_MS / 1000, ease: "linear" };

export const QUICK: Transition = { duration: 0.22, ease: [0.22, 1, 0.36, 1] };

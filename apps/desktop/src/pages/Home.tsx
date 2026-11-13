import type { ReactNode } from "react";
import { motion, useReducedMotion } from "motion/react";
import type { IconSvgElement } from "@hugeicons/react";
import {
  AiBrain01Icon,
  DashboardSpeed01Icon,
  Mic01Icon,
  SparklesIcon,
  Timer01Icon,
  TranslateIcon,
} from "@hugeicons/core-free-icons";
import { useTranslation } from "react-i18next";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  Cell,
  PolarAngleAxis,
  RadialBar,
  RadialBarChart,
  XAxis,
} from "recharts";
import { Page } from "@/components/layout/Page";
import { IconChip, type Tone } from "@/components/shared/IconChip";
import { Keys } from "@/components/shared/Keys";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import { Separator } from "@/components/ui/separator";
import { useHome } from "@/hooks/useHome";
import { formatNumber, formatPercent, formatWeekday, spanParts } from "@/lib/format";
import { cn } from "@/lib/utils";

const EASE = [0.22, 1, 0.36, 1] as const;

// The small charts carry no legend or tooltip, so they have nothing to name.
const SPARK = {} satisfies ChartConfig;

function Rise({
  index,
  className,
  children,
}: {
  index: number;
  className?: string;
  children: ReactNode;
}) {
  return (
    <motion.div
      className={cn("min-w-0", className)}
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4, delay: index * 0.05, ease: EASE }}
    >
      {children}
    </motion.div>
  );
}

function Span({ milliseconds, language }: { milliseconds: number; language: string }) {
  return (
    <span className="flex items-baseline gap-1">
      {spanParts(milliseconds, language).map(({ amount, unit }) => (
        <span key={unit}>
          {amount}
          <span className="ml-0.5 text-[0.45em] font-medium opacity-70">{unit}</span>
        </span>
      ))}
    </span>
  );
}

function Waveform() {
  return (
    <div aria-hidden className="waveform flex h-8 shrink-0 items-end gap-[3px]">
      {[0.5, 0.85, 0.6, 1, 0.7, 0.45].map((height, index) => (
        <span
          key={index}
          style={{ height: `${height * 100}%`, animationDelay: `${index * 90}ms` }}
        />
      ))}
    </div>
  );
}

function Sparkline({ values }: { values: number[] }) {
  const reduceMotion = useReducedMotion();
  const data = values.map((words, index) => ({ index, words }));

  return (
    <ChartContainer config={SPARK} className="aspect-auto h-14 w-full" aria-hidden>
      <AreaChart data={data} margin={{ top: 4, right: 0, bottom: 0, left: 0 }}>
        <defs>
          <linearGradient id="spark-fill" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="var(--spark)" stopOpacity="0.38" />
            <stop offset="100%" stopColor="var(--spark)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <Area
          dataKey="words"
          type="monotone"
          stroke="var(--spark)"
          strokeWidth={2.5}
          fill="url(#spark-fill)"
          isAnimationActive={!reduceMotion}
        />
      </AreaChart>
    </ChartContainer>
  );
}

function Ring({ share, label }: { share: number; label: string }) {
  const reduceMotion = useReducedMotion();

  return (
    <div className="relative size-[74px] shrink-0">
      <ChartContainer config={SPARK} className="aspect-square size-full" aria-hidden>
        <RadialBarChart
          data={[{ share }]}
          innerRadius="78%"
          outerRadius="100%"
          startAngle={90}
          endAngle={-270}
        >
          <PolarAngleAxis type="number" domain={[0, 1]} tick={false} />
          <RadialBar
            dataKey="share"
            cornerRadius={99}
            background={{ className: "ring-track" }}
            className="ring-value"
            isAnimationActive={!reduceMotion}
          />
        </RadialBarChart>
      </ChartContainer>
      <span className="absolute inset-0 grid place-items-center text-[13px] font-bold tabular-nums">
        {label}
      </span>
    </div>
  );
}

function StripFigure({
  icon,
  tone,
  label,
  value,
  note,
}: {
  icon: IconSvgElement;
  tone: Tone;
  label: string;
  value: ReactNode;
  note: string;
}) {
  return (
    <Item size="sm" className="min-w-0 flex-1 flex-nowrap gap-3 rounded-none border-0 px-4 py-3.5">
      <ItemMedia>
        <IconChip icon={icon} tone={tone} />
      </ItemMedia>
      <ItemContent className="min-w-0 gap-0">
        <ItemDescription className="truncate text-[12px]">{label}</ItemDescription>
        <ItemTitle className="flex items-baseline gap-1.5">
          <span className="text-xl leading-7 font-bold tracking-[-0.03em] tabular-nums">
            {value}
          </span>
          <span className="truncate text-[11px] font-normal text-muted-foreground">{note}</span>
        </ItemTitle>
      </ItemContent>
    </Item>
  );
}

export function Home() {
  const { state } = useHome();
  const { t, i18n } = useTranslation();
  const reduceMotion = useReducedMotion();
  const language = i18n.language;
  const chart = { words: { label: t("home.stats.words.label") } } satisfies ChartConfig;
  const today = state.days.length - 1;
  const [dictate, ...others] = state.modes;
  const typingMs = state.savedMs + state.durationMs;
  const savedShare = typingMs > 0 ? state.savedMs / typingMs : 0;

  return (
    <Page title={t("nav.home")}>
      <section className="flex flex-col gap-2.5">
        <div className="flex h-5 items-center justify-between gap-3 px-1">
          <h2 className="text-[13px] font-semibold text-muted-foreground">
            {t("home.shortcuts.title")}
          </h2>
          <p className="text-[12px] text-muted-foreground">{t("home.shortcuts.caption")}</p>
        </div>
        <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)] gap-3">
          <Rise index={0}>
            <div className="h-full">
              <Card className="mode-hero h-full min-h-[160px] gap-0 px-4 py-4 text-white">
                <div className="flex items-start justify-between gap-2">
                  <IconChip icon={Mic01Icon} tone="glass" className="size-11" />
                  <div className="hero-keys">
                    <Keys keys={dictate.keys} />
                  </div>
                </div>
                <div className="mt-auto flex items-end justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="text-[19px] font-bold tracking-[-0.02em]">
                      {t(`home.modes.${dictate.id}.name`)}
                    </h3>
                    <p className="mt-0.5 line-clamp-2 text-[12.5px] text-white/80">
                      {t(`home.modes.${dictate.id}.description`)}
                    </p>
                  </div>
                  <Waveform />
                </div>
              </Card>
            </div>
          </Rise>
          <div className="grid min-w-0 grid-rows-2 gap-3">
            {others.map(({ id, keys }, index) => (
              <Rise key={id} index={index + 1}>
                <div className="h-full">
                  <Item
                    variant="outline"
                    size="sm"
                    className={cn("mode-mini h-full flex-nowrap rounded-[20px]", `mode-${id}`)}
                  >
                    <ItemMedia>
                      <IconChip
                        icon={id === "translate" ? TranslateIcon : SparklesIcon}
                        tone={id === "translate" ? "cyan" : "violet"}
                      />
                    </ItemMedia>
                    <ItemContent className="min-w-0 gap-0">
                      <ItemTitle className="truncate">{t(`home.modes.${id}.name`)}</ItemTitle>
                      <ItemDescription className="truncate text-[11.5px]">
                        {t(`home.modes.${id}.description`)}
                      </ItemDescription>
                    </ItemContent>
                    <ItemActions>
                      <Keys keys={keys} />
                    </ItemActions>
                  </Item>
                </div>
              </Rise>
            ))}
          </div>
        </div>
      </section>

      <section className="flex flex-col gap-2.5">
        <div className="flex h-5 items-center justify-between gap-3 px-1">
          <h2 className="text-[13px] font-semibold text-muted-foreground">
            {t("home.overview.title")}
          </h2>
          <p className="text-[12px] text-muted-foreground">{t("home.overview.period")}</p>
        </div>

        <div className="grid grid-cols-5 gap-3">
          <Rise index={3} className="col-span-3">
            <Card className="stat-ink h-full min-h-[132px] gap-0 px-4 py-4 text-white">
              <p className="text-[12.5px] font-medium text-white/70">
                {t("home.stats.words.label")}
              </p>
              <div className="mt-1 flex items-baseline gap-1.5">
                <span className="text-[38px] leading-[44px] font-bold tracking-[-0.045em] tabular-nums">
                  {formatNumber(state.words, language)}
                </span>
                <span className="text-[12px] text-white/65">{t("home.stats.words.note")}</span>
              </div>
              <div className="mt-auto">
                <Sparkline values={state.days.map(({ words }) => words)} />
              </div>
            </Card>
          </Rise>

          <Rise index={4} className="col-span-2">
            <Card className="h-full min-h-[132px] justify-between gap-0 px-4 py-4">
              <p className="text-[12.5px] font-medium text-muted-foreground">
                {t("home.stats.saved.label")}
              </p>
              <div className="flex items-end justify-between gap-2">
                <div className="min-w-0">
                  <div className="text-[28px] leading-8 font-bold tracking-[-0.04em] tabular-nums">
                    <Span milliseconds={state.savedMs} language={language} />
                  </div>
                  <p className="mt-1 text-[11.5px] text-muted-foreground">
                    {t("home.stats.saved.note")}
                  </p>
                </div>
                <Ring share={savedShare} label={formatPercent(savedShare, language)} />
              </div>
            </Card>
          </Rise>
        </div>

        <Rise index={5}>
          <Card size="sm" className="flex-row gap-0 py-0">
            <StripFigure
              icon={Timer01Icon}
              tone="blue"
              label={t("home.stats.time.label")}
              value={<Span milliseconds={state.durationMs} language={language} />}
              note={t("home.stats.time.note", { n: formatNumber(state.sessions, language) })}
            />
            <Separator orientation="vertical" className="my-3 bg-foreground/6" />
            <StripFigure
              icon={DashboardSpeed01Icon}
              tone="pink"
              label={t("home.stats.pace.label")}
              value={formatNumber(state.pace, language)}
              note={t("home.stats.pace.note")}
            />
            <Separator orientation="vertical" className="my-3 bg-foreground/6" />
            <StripFigure
              icon={AiBrain01Icon}
              tone="green"
              label={t("home.stats.learned.label")}
              value={formatNumber(state.learned, language)}
              note={t("home.stats.learned.note")}
            />
          </Card>
        </Rise>

        <Rise index={6}>
          <Card size="sm" className="gap-3 px-4">
            <div className="flex items-baseline gap-2">
              <h3 className="text-sm font-semibold">{t("home.chart.title")}</h3>
              <p className="min-w-0 flex-1 text-xs text-muted-foreground">
                {t("home.chart.period")}
              </p>
              {state.trend !== null && (
                <Badge
                  variant="secondary"
                  data-direction={state.trend >= 0 ? "up" : "down"}
                  className="trend-pill h-6 px-2.5 font-normal"
                >
                  <span className="font-semibold tabular-nums">
                    {formatPercent(state.trend, language, { signed: true })}
                  </span>
                  {t("home.chart.trend")}
                </Badge>
              )}
            </div>
            <ChartContainer config={chart} className="aspect-auto h-32 w-full">
              <BarChart data={state.days} margin={{ top: 4, right: 0, bottom: 0, left: 0 }}>
                <defs>
                  <linearGradient id="bar-today" x1="0" x2="0" y1="0" y2="1">
                    <stop offset="0%" stopColor="var(--tone-orange)" />
                    <stop offset="100%" stopColor="var(--tone-pink)" />
                  </linearGradient>
                </defs>
                <XAxis
                  dataKey="day"
                  tickLine={false}
                  axisLine={false}
                  tick={{ fill: "var(--muted-foreground)" }}
                  tickMargin={6}
                  tickFormatter={(day: number) => formatWeekday(day, language)}
                />
                <ChartTooltip cursor={false} content={<ChartTooltipContent hideLabel />} />
                <Bar
                  dataKey="words"
                  radius={[8, 8, 8, 8]}
                  maxBarSize={30}
                  isAnimationActive={!reduceMotion}
                  animationDuration={650}
                  animationEasing="ease-out"
                >
                  {state.days.map(({ day }, index) => (
                    <Cell key={day} fill={index === today ? "url(#bar-today)" : "var(--chart-2)"} />
                  ))}
                </Bar>
              </BarChart>
            </ChartContainer>
          </Card>
        </Rise>
      </section>
    </Page>
  );
}

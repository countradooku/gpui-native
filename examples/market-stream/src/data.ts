export type MarketHealth = "NORMAL" | "HOT" | "LAG"

export interface MarketRow {
  id: number
  symbol: string
  company: string
  sector: string
  venue: string
  price: number
  bid: number
  ask: number
  change: number
  volume: number
  trades: number
  latency: number
  health: MarketHealth
}

export const sectors = [
  "Compute",
  "Energy",
  "Finance",
  "Health",
  "Industry",
  "Networks",
  "Quantum",
  "Space",
] as const

export const venues = ["ARCX", "BATS", "EDGX", "IEXG", "MEMX", "XNAS", "XNYS"] as const

const roots = [
  "AERO",
  "ALTA",
  "APEX",
  "ATOM",
  "AXIS",
  "BOLT",
  "CORE",
  "CYGN",
  "DASH",
  "ECHO",
  "FLUX",
  "HALO",
  "ION",
  "KERN",
  "LUMA",
  "MESH",
  "NOVA",
  "OMNI",
  "ORBT",
  "PRSM",
  "QBIT",
  "RIFT",
  "SOLR",
  "TENS",
  "ULTR",
  "VECT",
  "WAVE",
  "XENO",
] as const

const adjectives = [
  "Adaptive",
  "Advanced",
  "Applied",
  "Autonomous",
  "Continental",
  "Dynamic",
  "Global",
  "Integrated",
  "Next",
  "Precision",
  "Prime",
  "Unified",
] as const

const nouns = [
  "Analytics",
  "Capital",
  "Dynamics",
  "Energy",
  "Fabrication",
  "Health",
  "Industries",
  "Networks",
  "Robotics",
  "Systems",
  "Technologies",
  "Ventures",
] as const

function nextRandom(state: { value: number }): number {
  state.value = (Math.imul(state.value, 1_664_525) + 1_013_904_223) >>> 0
  return state.value / 4_294_967_296
}

export function generateMarket(count: number): MarketRow[] {
  const state = { value: 0x7e57_1ab5 }
  const rows: MarketRow[] = []

  for (let index = 0; index < count; index += 1) {
    const price = 7 + nextRandom(state) * 1_240
    const spread = Math.max(0.01, price * (0.00005 + nextRandom(state) * 0.00022))
    const latency = 0.18 + nextRandom(state) * 5.4
    rows.push({
      id: index,
      symbol: `${roots[index % roots.length]}${Math.floor(index / roots.length)
        .toString(36)
        .toUpperCase()
        .padStart(3, "0")}`,
      company: `${adjectives[(index * 7) % adjectives.length]} ${nouns[(index * 11) % nouns.length]}`,
      sector: sectors[(index * 5) % sectors.length] ?? sectors[0],
      venue: venues[(index * 13) % venues.length] ?? venues[0],
      price,
      bid: price - spread / 2,
      ask: price + spread / 2,
      change: (nextRandom(state) - 0.48) * 4.8,
      volume: Math.floor(15_000 + nextRandom(state) * 9_800_000),
      trades: Math.floor(80 + nextRandom(state) * 82_000),
      latency,
      health: latency > 5.15 ? "LAG" : "NORMAL",
    })
  }

  return rows
}

export function mixEntropy(value: number): number {
  let next = value | 0
  next ^= next << 13
  next ^= next >>> 17
  next ^= next << 5
  return next >>> 0
}

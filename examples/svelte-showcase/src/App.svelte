<script lang="ts">
  import {
    Button,
    Column,
    View,
    Row,
    Text,
    TextInput,
    Code,
    Markdown,
    Canvas,
    VirtualList,
    MotionView,
    Select,
    SelectTrigger,
    SelectValue,
    SelectContent,
    SelectItem,
    Combobox,
    ComboboxInput,
    ComboboxContent,
    ComboboxItem,
    ComboboxEmpty,
    Tooltip,
    TooltipTrigger,
    TooltipContent,
    useTextSearch,
  } from "@gpui-native/svelte";
  let language = $state("typescript"),
    fruit = $state(""),
    query = $state("native");
  const search = useTextSearch(() => ({ query }));
  const panel = { padding: 12, background: "#1e293b", borderRadius: 8 };
</script>

<Column
  style={{
    width: "100%",
    height: "100%",
    padding: 24,
    gap: 12,
    background: "#111827",
    color: "#f8fafc",
    overflow: "scroll",
  }}
>
  <Text style={{ fontSize: 28 }}>Svelte native components</Text>
  <Row style={{ gap: 16 }}>
    <Tooltip
      ><TooltipTrigger style={panel}>Hover or focus</TooltipTrigger
      ><TooltipContent style={panel}>A native GPUI tooltip</TooltipContent
      ></Tooltip
    >
    <Select bind:value={language}
      ><SelectTrigger style={panel}><SelectValue /></SelectTrigger
      ><SelectContent style={panel}
        ><SelectItem value="typescript" textValue="TypeScript"
          >TypeScript</SelectItem
        ><SelectItem value="rust" textValue="Rust">Rust</SelectItem
        ></SelectContent
      ></Select
    >
    <Combobox bind:value={fruit}
      ><ComboboxInput placeholder="Find fruit" style={panel} /><ComboboxContent
        style={panel}
        ><ComboboxItem value="apple" textValue="Apple">Apple</ComboboxItem
        ><ComboboxItem value="banana" textValue="Banana">Banana</ComboboxItem
        ><ComboboxEmpty>Nothing found</ComboboxEmpty></ComboboxContent
      ></Combobox
    >
  </Row>
  <Code
    code={language === "rust" ? "let count = 42;" : "let count = $state(42)"}
    {language}
    style={panel}
  />
  <Row style={{ gap: 8 }}
    ><TextInput
      bind:value={query}
      placeholder="Search native text"
      style={panel}
    /><Button onPress={() => search.next()} style={panel}
      >Next match ({search.total})</Button
    ></Row
  >
  <View {...search.props} style={panel}>
    <Text>Native text uses GPUI selection and native highlighting.</Text>
  </View>
  <Markdown
    source={"# Markdown\nSvelte owns reactivity. GPUI owns **text, layout and paint**."}
    style={panel}
  />
  <Canvas
    commands={[
      { type: "rect", x: 0, y: 0, width: 180, height: 40, fill: "#60a5fa" },
    ]}
    style={{ height: 48, flexShrink: 0 }}
  />
  <MotionView
    initial={{ opacity: 0, left: -20 }}
    animate={{ opacity: 1, left: 0 }}
    transition={{ duration: 0.5 }}
    ><Text>Animation runs in GPUI</Text></MotionView
  >
  <VirtualList estimatedItemHeight={28} style={{ height: 180, flexShrink: 0 }}
    >{#each Array.from({ length: 1000 }, (_, i) => i) as id (id)}<Text
        style={{ height: 28 }}>Native row {id}</Text
      >{/each}</VirtualList
  >
</Column>

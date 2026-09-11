<script lang="ts">
  import {
    Button,
    Column,
    Row,
    Text,
    TextInput,
    useWindowSize,
  } from "@gpui-native/svelte";
  let count = $state(0),
    name = $state("Svelte");
  const doubled = $derived(count * 2),
    size = useWindowSize();
  const button = { padding: 14, background: "#30466e", borderRadius: 8 };
</script>

<Column
  style={{
    width: "100%",
    height: "100%",
    padding: 32,
    gap: 20,
    background: "#111827",
    color: "#f8fafc",
  }}
>
  <Text style={{ fontSize: 30, fontWeight: 700 }}>Hello, {name}</Text>
  <TextInput
    testId="name"
    bind:value={name}
    style={{ width: 280, padding: 12, background: "#1e293b" }}
  />
  <Text testId="count" style={{ fontSize: 48 }}>{count}</Text>
  <Text>Derived with runes: {doubled}</Text>
  <Row style={{ gap: 12 }}
    ><Button testId="decrement" style={button} onPress={() => count--}>−</Button
    ><Button testId="increment" style={button} onPress={() => count++}>+</Button
    ><Button
      style={{ ...button, opacity: count === 0 ? 0.5 : 1 }}
      disabled={count === 0}
      onPress={() => (count = 0)}>Reset</Button
    ></Row
  >
  <Text>{size.width} × {size.height} · Svelte runes, GPUI rendering</Text>
</Column>

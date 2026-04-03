<template>
  <div ref="termEl" class="w-full h-full" />
</template>

<script setup lang="ts">
import { ref, onMounted, watch, onUnmounted } from 'vue'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import type { PrometheusResponse } from '@/types'

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()
const termEl = ref<HTMLElement>()
let term: Terminal | null = null

function render() {
  if (!termEl.value) return
  if (!term) {
    term = new Terminal({ disableStdin: true, convertEol: true })
    term.open(termEl.value)
  }
  term.clear()

  const streams = (props.data as PrometheusResponse)?.data?.result || []
  for (const stream of streams) {
    for (const [, line] of stream.values || []) {
      term.writeln(line)
    }
  }
}

onMounted(render)
watch(() => props.data, render)
onUnmounted(() => term?.dispose())
</script>

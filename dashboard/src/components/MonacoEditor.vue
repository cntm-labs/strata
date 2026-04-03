<template>
  <div
    ref="editorEl"
    class="w-full border border-base-300 rounded"
    :style="{ height: height + 'px' }"
  />
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import * as monaco from 'monaco-editor'

const props = withDefaults(
  defineProps<{
    modelValue: string
    language?: string
    height?: number
  }>(),
  { language: 'promql', height: 120 },
)

const emit = defineEmits<{ 'update:modelValue': [value: string] }>()

const editorEl = ref<HTMLElement>()
let editor: monaco.editor.IStandaloneCodeEditor | null = null

onMounted(() => {
  if (!editorEl.value) return
  editor = monaco.editor.create(editorEl.value, {
    value: props.modelValue,
    language: props.language,
    theme: 'vs-dark',
    minimap: { enabled: false },
    lineNumbers: 'off',
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    fontSize: 13,
    tabSize: 2,
    automaticLayout: true,
  })

  editor.onDidChangeModelContent(() => {
    emit('update:modelValue', editor!.getValue())
  })
})

watch(
  () => props.modelValue,
  (val) => {
    if (editor && editor.getValue() !== val) {
      editor.setValue(val)
    }
  },
)

onUnmounted(() => editor?.dispose())
</script>

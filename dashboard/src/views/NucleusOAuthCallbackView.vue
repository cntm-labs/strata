<template>
  <div class="p-4 text-center">Completing sign-in…</div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'

onMounted(() => {
  const params = new URLSearchParams(window.location.search)
  const message = {
    type: 'nucleus:oauth:callback',
    code: params.get('code'),
    state: params.get('state'),
    error: params.get('error'),
  }
  if (window.opener) {
    window.opener.postMessage(message, window.location.origin)
  }
  window.close()
})
</script>

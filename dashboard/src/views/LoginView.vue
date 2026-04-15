<template>
  <div class="flex items-center justify-center h-screen">
    <div class="text-center">
      <h1 class="text-2xl font-bold mb-4">Redirecting to login...</h1>
      <ProgressSpinner />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import ProgressSpinner from 'primevue/progressspinner'

onMounted(() => {
  const nucleusUrl = import.meta.env.VITE_NUCLEUS_URL || 'https://nucleus.example.com'
  const projectId = import.meta.env.VITE_NUCLEUS_PROJECT_ID || ''
  const redirectUri = `${window.location.origin}/auth/callback`
  window.location.href = `${nucleusUrl}/auth/sign-in?project_id=${projectId}&redirect_uri=${encodeURIComponent(redirectUri)}`
})
</script>

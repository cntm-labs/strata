<template>
  <div class="flex items-center justify-center h-screen">
    <ProgressSpinner />
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAuth } from '@/composables/useAuth'
import ProgressSpinner from 'primevue/progressspinner'

const router = useRouter()
const { setToken } = useAuth()

onMounted(() => {
  const params = new URLSearchParams(window.location.search)
  const token = params.get('token') || params.get('access_token')
  if (token) {
    setToken(token)
    router.replace('/dashboards')
  } else {
    router.replace('/login')
  }
})
</script>

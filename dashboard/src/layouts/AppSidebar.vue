<template>
  <aside class="w-64 bg-base-200 flex flex-col border-r border-base-300">
    <div class="p-4 text-xl font-bold">Strata</div>
    <nav class="flex-1 p-2">
      <ul class="menu">
        <li v-for="item in navItems" :key="item.path">
          <RouterLink :to="item.path" class="flex items-center gap-2">
            <i :class="item.icon" />
            <span>{{ item.label }}</span>
          </RouterLink>
        </li>
      </ul>
    </nav>
    <div v-if="user" class="p-4 border-t border-base-300">
      <div class="flex items-center gap-3">
        <div class="avatar placeholder">
          <div class="bg-neutral text-neutral-content w-8 rounded-full">
            <img v-if="user.avatar_url" :src="user.avatar_url" />
            <span v-else>{{ initials }}</span>
          </div>
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-sm font-medium truncate">{{ user.first_name || user.email }}</div>
        </div>
        <button class="btn btn-ghost btn-xs" @click="logout">
          <i class="pi pi-sign-out" />
        </button>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useAuth } from '@/composables/useAuth'

const router = useRouter()
const { user, signOut } = useAuth()

const initials = computed(() => {
  if (!user.value) return ''
  const first = user.value.first_name?.[0] ?? ''
  const last = user.value.last_name?.[0] ?? ''
  return (first + last).toUpperCase() || user.value.email?.[0]?.toUpperCase() || '?'
})

async function logout() {
  await signOut()
  router.push('/login')
}

const navItems = [
  { path: '/dashboards', label: 'Dashboards', icon: 'pi pi-th-large' },
  { path: '/explore', label: 'Explore', icon: 'pi pi-search' },
  { path: '/alerts', label: 'Alerts', icon: 'pi pi-bell' },
  { path: '/datasources', label: 'Data Sources', icon: 'pi pi-database' },
  { path: '/templates', label: 'Templates', icon: 'pi pi-copy' },
  { path: '/settings', label: 'Settings', icon: 'pi pi-cog' },
]
</script>

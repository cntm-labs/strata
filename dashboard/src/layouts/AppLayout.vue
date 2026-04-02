<template>
  <div class="flex h-screen bg-base-100">
    <Sidebar />
    <main class="flex-1 overflow-auto p-6">
      <RouterView />
    </main>
  </div>
</template>

<script setup lang="ts">
import Sidebar from "./Sidebar.vue";
</script>

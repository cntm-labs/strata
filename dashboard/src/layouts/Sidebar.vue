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
  </aside>
</template>

<script setup lang="ts">
const navItems = [
  { path: "/dashboards", label: "Dashboards", icon: "pi pi-th-large" },
  { path: "/explore", label: "Explore", icon: "pi pi-search" },
  { path: "/alerts", label: "Alerts", icon: "pi pi-bell" },
  { path: "/datasources", label: "Data Sources", icon: "pi pi-database" },
  { path: "/templates", label: "Templates", icon: "pi pi-copy" },
  { path: "/settings", label: "Settings", icon: "pi pi-cog" },
];
</script>

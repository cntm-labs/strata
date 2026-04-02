import { createRouter, createWebHistory } from "vue-router";
import AppLayout from "@/layouts/AppLayout.vue";

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: "/",
      component: AppLayout,
      children: [
        { path: "", redirect: "/dashboards" },
        {
          path: "dashboards",
          name: "dashboards",
          component: () => import("@/views/DashboardListView.vue"),
        },
        {
          path: "dashboards/new",
          name: "dashboard-new",
          component: () => import("@/views/DashboardNewView.vue"),
        },
        {
          path: "dashboards/:slug",
          name: "dashboard-view",
          component: () => import("@/views/DashboardView.vue"),
        },
        {
          path: "dashboards/:slug/edit",
          name: "dashboard-edit",
          component: () => import("@/views/DashboardEditView.vue"),
        },
        {
          path: "explore",
          name: "explore",
          component: () => import("@/views/ExploreView.vue"),
        },
        {
          path: "alerts",
          name: "alerts",
          component: () => import("@/views/AlertsView.vue"),
        },
        {
          path: "alerts/rules/new",
          name: "alert-rule-new",
          component: () => import("@/views/AlertRuleEditView.vue"),
        },
        {
          path: "alerts/rules/:id",
          name: "alert-rule-edit",
          component: () => import("@/views/AlertRuleEditView.vue"),
        },
        {
          path: "alerts/events",
          name: "alert-events",
          component: () => import("@/views/AlertEventsView.vue"),
        },
        {
          path: "datasources",
          name: "datasources",
          component: () => import("@/views/DatasourceListView.vue"),
        },
        {
          path: "datasources/new",
          name: "datasource-new",
          component: () => import("@/views/DatasourceEditView.vue"),
        },
        {
          path: "datasources/:id",
          name: "datasource-edit",
          component: () => import("@/views/DatasourceEditView.vue"),
        },
        {
          path: "templates",
          name: "templates",
          component: () => import("@/views/TemplatesView.vue"),
        },
        {
          path: "settings",
          name: "settings",
          component: () => import("@/views/SettingsView.vue"),
        },
      ],
    },
  ],
});

export default router;

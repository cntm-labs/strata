import { describe, it, expect } from 'vitest'
import { shallowMount } from '@vue/test-utils'
import AppLayout from '../AppLayout.vue'

describe('AppLayout', () => {
  it('renders sidebar and router-view', () => {
    const wrapper = shallowMount(AppLayout, {
      global: {
        stubs: {
          RouterView: true,
          AppSidebar: true,
        },
      },
    })
    expect(wrapper.find('main').exists()).toBe(true)
    expect(wrapper.findComponent({ name: 'AppSidebar' }).exists()).toBe(true)
    expect(wrapper.findComponent({ name: 'RouterView' }).exists()).toBe(true)
  })
})

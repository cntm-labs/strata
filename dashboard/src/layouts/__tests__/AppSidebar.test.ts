import { describe, it, expect } from 'vitest'
import { shallowMount } from '@vue/test-utils'
import AppSidebar from '../AppSidebar.vue'

describe('AppSidebar', () => {
  it('renders Strata brand', () => {
    const wrapper = shallowMount(AppSidebar, {
      global: { stubs: { RouterLink: true } },
    })
    expect(wrapper.text()).toContain('Strata')
  })

  it('renders all 6 nav links', () => {
    const wrapper = shallowMount(AppSidebar, {
      global: { stubs: { RouterLink: true } },
    })
    const links = wrapper.findAllComponents({ name: 'RouterLink' })
    expect(links.length).toBe(6)
  })

  it('has correct nav paths', () => {
    const wrapper = shallowMount(AppSidebar, {
      global: { stubs: { RouterLink: true } },
    })
    const links = wrapper.findAllComponents({ name: 'RouterLink' })
    const paths = links.map((l) => l.attributes('to'))
    expect(paths).toContain('/dashboards')
    expect(paths).toContain('/explore')
    expect(paths).toContain('/alerts')
    expect(paths).toContain('/datasources')
    expect(paths).toContain('/templates')
    expect(paths).toContain('/settings')
  })
})

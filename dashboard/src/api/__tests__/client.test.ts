import { describe, it, expect, vi, beforeEach } from 'vitest'

const mockFetch = vi.fn()
vi.stubGlobal('fetch', mockFetch)

const { api } = await import('../client')

beforeEach(() => {
  mockFetch.mockReset()
})

describe('api client', () => {
  describe('get', () => {
    it('sends GET request with correct headers', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: 'test' }),
      })

      const result = await api.get('/dashboards')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: { 'Content-Type': 'application/json' },
      })
      expect(result).toEqual({ data: 'test' })
    })
  })

  describe('post', () => {
    it('sends POST with JSON body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ id: '1' }),
      })

      await api.post('/dashboards', { title: 'New' })

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
        body: JSON.stringify({ title: 'New' }),
      })
    })

    it('sends POST without body when no data', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({}),
      })

      await api.post('/dashboards/slug/star')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug/star', {
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
        body: undefined,
      })
    })
  })

  describe('put', () => {
    it('sends PUT with JSON body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ id: '1' }),
      })

      await api.put('/dashboards/slug', { title: 'Updated' })

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug', {
        headers: { 'Content-Type': 'application/json' },
        method: 'PUT',
        body: JSON.stringify({ title: 'Updated' }),
      })
    })
  })

  describe('delete', () => {
    it('sends DELETE request', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(null),
      })

      await api.delete('/dashboards/slug')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug', {
        headers: { 'Content-Type': 'application/json' },
        method: 'DELETE',
      })
    })
  })

  describe('error handling', () => {
    it('throws with server error message', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ message: 'Not found' }),
      })

      await expect(api.get('/missing')).rejects.toThrow('Not found')
    })

    it('throws with status text when JSON parse fails', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
        json: () => Promise.reject(new Error('invalid json')),
      })

      await expect(api.get('/broken')).rejects.toThrow('Internal Server Error')
    })

    it('throws generic message when no message in error body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 400,
        json: () => Promise.resolve({}),
      })

      await expect(api.get('/bad')).rejects.toThrow('HTTP 400')
    })
  })
})

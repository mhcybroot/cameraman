<script setup>
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'

const events = ref([])
const selectedEvent = ref(null)
const autoRefresh = ref(true)
const refreshCountdown = ref(3)
let refreshInterval = null

const filter = ref({
  search: '',
  status: 'all',
  district: 'all'
})

// Fetch database records
const fetchEvents = async () => {
  try {
    const response = await fetch('/api/events')
    if (response.ok) {
      const data = await response.json()
      events.value = data
      
      // Keep selection if it exists and is still in list
      if (selectedEvent.value) {
        const found = data.find(e => e.id === selectedEvent.value.id)
        if (found) {
          selectedEvent.value = found
        }
      } else if (data.length > 0 && !selectedEvent.value) {
        selectedEvent.value = data[0]
      }
    }
  } catch (error) {
    console.error('Failed to fetch events:', error)
  } finally {
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

// Compute overall statistics
const stats = computed(() => {
  const total = events.value.length
  const valid = events.value.filter(e => e.is_plate_valid).length
  const rate = total > 0 ? Math.round((valid / total) * 100) : 0
  return { total, valid, rate }
})

// Find list of districts present in current dataset for filters
const availableDistricts = computed(() => {
  const districts = new Set()
  events.value.forEach(e => {
    if (e.district) {
      districts.add(e.district)
    }
  })
  return Array.from(districts).sort()
})

// Filter logic
const filteredEvents = computed(() => {
  return events.value.filter(event => {
    // Camera filter
    if (cameraFilter.value !== 'all' && event.camera_id !== cameraFilter.value) {
      return false
    }

    // Search filter
    const searchLower = filter.value.search.trim().toLowerCase()
    if (searchLower) {
      const num = event.plate_number || ''
      if (!num.toLowerCase().includes(searchLower)) {
        return false
      }
    }

    // Status filter
    if (filter.value.status === 'valid' && !event.is_plate_valid) {
      return false
    }
    if (filter.value.status === 'invalid' && event.is_plate_valid) {
      return false
    }

    // District filter
    if (filter.value.district !== 'all' && event.district !== filter.value.district) {
      return false
    }

    return true
  })
})

const selectEvent = (event) => {
  selectedEvent.value = event
  nextTick(() => {
    if (window.lucide) {
      window.lucide.createIcons()
    }
  })
}

const formatTime = (timeStr) => {
  if (!timeStr) return ''
  const d = new Date(timeStr)
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

const formatFullDateTime = (timeStr) => {
  if (!timeStr) return ''
  const d = new Date(timeStr)
  return d.toLocaleString()
}

// Handle refresh interval
const startTimer = () => {
  refreshCountdown.value = 3
  refreshInterval = setInterval(async () => {
    if (autoRefresh.value) {
      refreshCountdown.value--
      if (refreshCountdown.value <= 0) {
        await fetchEvents()
        refreshCountdown.value = 3
      }
    }
  }, 1000)
}

const activeTab = ref('events') // 'events' or 'cameras'
const cameras = ref([])
const newCamera = ref({
  id: '',
  name: '',
  location: ''
})
const cameraFilter = ref('all')
const addCameraError = ref('')
const isAddingCamera = ref(false)

const fetchCameras = async () => {
  try {
    const response = await fetch('/api/cameras')
    if (response.ok) {
      cameras.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to fetch cameras:', error)
  }
}

const addCamera = async () => {
  if (!newCamera.value.name.trim()) {
    addCameraError.value = 'Camera Name is required'
    return
  }
  addCameraError.value = ''
  isAddingCamera.value = true
  try {
    const response = await fetch('/api/cameras', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        id: newCamera.value.id.trim() || undefined,
        name: newCamera.value.name.trim(),
        location: newCamera.value.location.trim() || undefined
      })
    })
    if (response.ok) {
      newCamera.value = { id: '', name: '', location: '' }
      await fetchCameras()
    } else {
      const err = await response.json()
      addCameraError.value = err.message || 'Failed to add camera'
    }
  } catch (error) {
    addCameraError.value = 'Network error adding camera'
    console.error(error)
  } finally {
    isAddingCamera.value = false
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

const deleteCamera = async (id) => {
  if (!confirm(`Are you sure you want to unregister camera ${id}?`)) {
    return
  }
  try {
    const response = await fetch(`/api/cameras/${id}`, {
      method: 'DELETE'
    })
    if (response.ok) {
      await fetchCameras()
    }
  } catch (error) {
    console.error('Failed to delete camera:', error)
  }
}

const getCameraName = (id) => {
  if (!id) return 'Unknown Cam'
  const cam = cameras.value.find(c => c.id === id)
  return cam ? cam.name : id
}

const getCameraLocation = (id) => {
  if (!id) return null
  const cam = cameras.value.find(c => c.id === id)
  return cam ? cam.location : null
}

const aiConfigs = ref([])
const newAiConfig = ref({
  name: '',
  provider_type: 'gemini',
  api_key: '',
  model_name: 'gemini-2.5-flash',
  is_active: false
})
const addAiConfigError = ref('')
const isAddingAiConfig = ref(false)

watch(() => newAiConfig.value.provider_type, (newType) => {
  if (newType === 'gemini') {
    newAiConfig.value.model_name = 'gemini-2.5-flash'
  } else if (newType === 'minimax') {
    newAiConfig.value.model_name = 'MiniMax-M3'
  } else {
    newAiConfig.value.model_name = ''
  }
})

const fetchAiConfigs = async () => {
  try {
    const response = await fetch('/api/ai-configs')
    if (response.ok) {
      aiConfigs.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to fetch AI configurations:', error)
  }
}

const addAiConfig = async () => {
  if (!newAiConfig.value.name.trim()) {
    addAiConfigError.value = 'Configuration Name is required'
    return
  }
  const isKeyRequired = newAiConfig.value.provider_type === 'gemini' || newAiConfig.value.provider_type === 'minimax'
  if (isKeyRequired && !newAiConfig.value.api_key.trim()) {
    addAiConfigError.value = `API Key is required for ${newAiConfig.value.provider_type === 'gemini' ? 'Gemini' : 'Minimax'}`
    return
  }

  addAiConfigError.value = ''
  isAddingAiConfig.value = true
  try {
    const response = await fetch('/api/ai-configs', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        name: newAiConfig.value.name.trim(),
        provider_type: newAiConfig.value.provider_type,
        api_key: isKeyRequired ? newAiConfig.value.api_key.trim() : undefined,
        model_name: newAiConfig.value.model_name.trim() || undefined,
        is_active: newAiConfig.value.is_active
      })
    })

    if (response.ok) {
      newAiConfig.value = {
        name: '',
        provider_type: 'gemini',
        api_key: '',
        model_name: 'gemini-2.5-flash',
        is_active: false
      }
      await fetchAiConfigs()
    } else {
      const err = await response.json()
      addAiConfigError.value = err.message || 'Failed to save configuration'
    }
  } catch (error) {
    addAiConfigError.value = 'Network error saving configuration'
    console.error(error)
  } finally {
    isAddingAiConfig.value = false
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

const deleteAiConfig = async (id) => {
  if (!confirm('Are you sure you want to delete this AI configuration?')) {
    return
  }
  try {
    const response = await fetch(`/api/ai-configs/${id}`, {
      method: 'DELETE'
    })
    if (response.ok) {
      await fetchAiConfigs()
    }
  } catch (error) {
    console.error('Failed to delete AI configuration:', error)
  }
}

const activateAiConfig = async (id) => {
  try {
    const response = await fetch(`/api/ai-configs/${id}/activate`, {
      method: 'POST'
    })
    if (response.ok) {
      await fetchAiConfigs()
    }
  } catch (error) {
    console.error('Failed to activate AI configuration:', error)
  }
}

watch(autoRefresh, (newVal) => {
  if (newVal) {
    refreshCountdown.value = 3
  }
})

watch(activeTab, () => {
  nextTick(() => {
    if (window.lucide) {
      window.lucide.createIcons()
    }
  })
})

onMounted(async () => {
  await fetchEvents()
  await fetchCameras()
  await fetchAiConfigs()
  startTimer()
})

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval)
  }
})
</script>

<template>
  <div class="app-container">
    <!-- Header -->
    <header>
      <div class="logo-section">
        <div class="logo-icon">
          <i data-lucide="eye" style="color: white; width: 22px; height: 22px;"></i>
        </div>
        <div>
          <h1 class="logo-text">CAMERAMAN</h1>
          <div style="font-size: 0.7rem; color: var(--text-muted); letter-spacing: 0.5px; text-transform: uppercase;">
            CCTV AI Recognition Ingestion Portal
          </div>
        </div>
      </div>

      <!-- Stats -->
      <div class="system-stats">
        <div class="stat-card">
          <span class="stat-val">{{ stats.total }}</span>
          <span class="stat-lbl">Total Events</span>
        </div>
        <div class="stat-card">
          <span class="stat-val" style="color: var(--status-success);">{{ stats.valid }}</span>
          <span class="stat-lbl">Valid Plates</span>
        </div>
        <div class="stat-card">
          <span class="stat-val" style="color: var(--status-warning);">{{ stats.rate }}%</span>
          <span class="stat-lbl">Detection Rate</span>
        </div>
      </div>
    </header>

    <!-- Main workspace -->
    <div class="dashboard-grid">
      <!-- Sidebar column -->
      <aside class="sidebar">
        <!-- Tab Switcher -->
        <div class="tabs-header">
          <button 
            class="tab-btn" 
            :class="{ active: activeTab === 'events' }" 
            @click="activeTab = 'events'"
          >
            <i data-lucide="activity" style="width: 16px; height: 16px;"></i>
            Events
          </button>
          <button 
            class="tab-btn" 
            :class="{ active: activeTab === 'cameras' }" 
            @click="activeTab = 'cameras'"
          >
            <i data-lucide="video" style="width: 16px; height: 16px;"></i>
            Cameras
          </button>
          <button 
            class="tab-btn" 
            :class="{ active: activeTab === 'ai' }" 
            @click="activeTab = 'ai'"
          >
            <i data-lucide="cpu" style="width: 16px; height: 16px;"></i>
            AI Config
          </button>
        </div>

        <!-- Tab 1: Events Feed -->
        <template v-if="activeTab === 'events'">
          <div class="sidebar-header">
            <h2 class="sidebar-title">
              Activity Feed
            </h2>
            <span class="badge badge-success" style="font-size: 0.6rem;">Live</span>
          </div>

          <!-- Controls -->
          <div class="search-box">
            <i data-lucide="search" style="width: 16px; height: 16px;"></i>
            <input type="text" placeholder="Search license plate digits..." v-model="filter.search">
          </div>

          <!-- Camera Dropdown Filter -->
          <div class="camera-filter-box">
            <select class="filter-select w-full" v-model="cameraFilter">
              <option value="all">All Connected Cameras</option>
              <option v-for="cam in cameras" :key="cam.id" :value="cam.id">
                {{ cam.name }} ({{ cam.id }})
              </option>
            </select>
          </div>

          <div class="filter-group">
            <select class="filter-select" v-model="filter.status">
              <option value="all">All Statuses</option>
              <option value="valid">Valid Only</option>
              <option value="invalid">Invalid Only</option>
            </select>
            <select class="filter-select" v-model="filter.district">
              <option value="all">All Districts</option>
              <option v-for="dist in availableDistricts" :key="dist" :value="dist">{{ dist }}</option>
            </select>
          </div>

          <div class="refresh-bar">
            <span style="display: flex; align-items: center; gap: 0.35rem;">
              <i data-lucide="refresh-cw" class="spin-icon" :class="{ paused: !autoRefresh }"></i>
              Auto Refresh ({{ refreshCountdown }}s)
            </span>
            <label class="switch">
              <input type="checkbox" v-model="autoRefresh">
              <span class="slider"></span>
            </label>
          </div>

          <!-- Events list container -->
          <div class="events-list">
            <div 
              v-for="event in filteredEvents" 
              :key="event.id"
              class="event-card"
              :class="{ active: selectedEvent && selectedEvent.id === event.id }"
              @click="selectEvent(event)"
            >
              <div class="event-card-header">
                <span class="event-card-cam">
                  <i data-lucide="video" style="width: 12px; height: 12px;"></i>
                  {{ getCameraName(event.camera_id) }}
                </span>
                <span>{{ formatTime(event.created_at) }}</span>
              </div>
              <div class="event-card-body">
                <div class="event-card-plate">
                  {{ event.plate_number || '— — —' }}
                </div>
                <span 
                  class="badge" 
                  :class="event.is_plate_valid ? 'badge-success' : 'badge-error'"
                >
                  {{ event.is_plate_valid ? 'Valid' : 'Invalid' }}
                </span>
              </div>
            </div>

            <div v-if="filteredEvents.length === 0" style="text-align: center; padding: 2rem; color: var(--text-muted);">
              No events found matching current criteria.
            </div>
          </div>
        </template>

        <!-- Tab 2: Cameras Manager -->
        <template v-else-if="activeTab === 'cameras'">
          <div class="sidebar-header">
            <h2 class="sidebar-title">
              Connected Cameras
            </h2>
            <span class="badge badge-success" style="font-size: 0.6rem;">{{ cameras.length }}</span>
          </div>

          <!-- Cameras list -->
          <div class="cameras-list">
            <div v-for="cam in cameras" :key="cam.id" class="camera-card">
              <div class="camera-card-header">
                <span class="camera-card-name">
                  <i data-lucide="video" style="width: 14px; height: 14px; color: var(--accent-primary);"></i>
                  {{ cam.name }}
                </span>
                <button class="delete-cam-btn" @click="deleteCamera(cam.id)">
                  <i data-lucide="trash-2" style="width: 14px; height: 14px;"></i>
                </button>
              </div>
              <div class="camera-card-body">
                <div class="camera-card-info">
                  <span class="lbl">ID:</span> <span class="val">{{ cam.id }}</span>
                </div>
                <div class="camera-card-info" v-if="cam.location">
                  <span class="lbl">Location:</span> <span class="val">{{ cam.location }}</span>
                </div>
              </div>
            </div>

            <div v-if="cameras.length === 0" style="text-align: center; padding: 2rem; color: var(--text-muted);">
              No cameras connected yet.
            </div>
          </div>

          <!-- Add Camera Form -->
          <div class="add-camera-form">
            <h3 class="form-title">Register New Camera</h3>
            
            <div class="form-group">
              <label>Camera ID (slug, optional)</label>
              <input type="text" placeholder="e.g. cam-east-01" v-model="newCamera.id">
            </div>

            <div class="form-group">
              <label>Camera Name *</label>
              <input type="text" placeholder="e.g. East Entrance Gate" v-model="newCamera.name">
            </div>

            <div class="form-group">
              <label>Location (optional)</label>
              <input type="text" placeholder="e.g. Floor 1, Lobby" v-model="newCamera.location">
            </div>

            <div v-if="addCameraError" class="form-error">
              {{ addCameraError }}
            </div>

            <button class="submit-btn" :disabled="isAddingCamera" @click="addCamera">
              <i data-lucide="plus-circle" style="width: 16px; height: 16px;"></i>
              {{ isAddingCamera ? 'Registering...' : 'Register Camera' }}
            </button>
          </div>
        </template>

        <!-- Tab 3: AI Configs Manager -->
        <template v-else-if="activeTab === 'ai'">
          <div class="sidebar-header">
            <h2 class="sidebar-title">
              AI Vision Configurations
            </h2>
            <span class="badge badge-success" style="font-size: 0.6rem;">{{ aiConfigs.length }}</span>
          </div>

          <!-- AI Configs list -->
          <div class="cameras-list">
            <div 
              v-for="config in aiConfigs" 
              :key="config.id" 
              class="camera-card"
              :style="config.is_active ? 'border-color: var(--status-success); background: rgba(16, 185, 129, 0.05);' : ''"
            >
              <div class="camera-card-header">
                <span class="camera-card-name" style="display: flex; flex-direction: column; align-items: flex-start; gap: 0.15rem;">
                  <span style="font-weight: 700; font-size: 0.95rem; color: var(--text-main);">
                    {{ config.name }}
                  </span>
                  <span class="badge" :class="config.is_active ? 'badge-success' : 'badge-error'" style="font-size: 0.55rem; padding: 0.1rem 0.35rem; margin-top: 0.15rem;">
                    {{ config.is_active ? 'Active' : 'Inactive' }}
                  </span>
                </span>
                
                <div style="display: flex; gap: 0.25rem;">
                  <button 
                    v-if="!config.is_active" 
                    class="delete-cam-btn" 
                    style="color: var(--status-success);" 
                    title="Activate Key" 
                    @click="activateAiConfig(config.id)"
                  >
                    <i data-lucide="check-circle" style="width: 14px; height: 14px;"></i>
                  </button>
                  <button class="delete-cam-btn" title="Delete Key" @click="deleteAiConfig(config.id)">
                    <i data-lucide="trash-2" style="width: 14px; height: 14px;"></i>
                  </button>
                </div>
              </div>
              
              <div class="camera-card-body">
                <div class="camera-card-info">
                  <span class="lbl">Provider:</span> 
                  <span class="val" style="text-transform: uppercase; font-weight: 700; font-size: 0.75rem;">
                    {{ config.provider_type }}
                  </span>
                </div>
                <div class="camera-card-info" v-if="config.model_name">
                  <span class="lbl">Model:</span> <span class="val">{{ config.model_name }}</span>
                </div>
                <div class="camera-card-info" v-if="config.api_key">
                  <span class="lbl">API Key:</span> <span class="val" style="font-family: monospace; font-size: 0.75rem;">{{ config.api_key }}</span>
                </div>
              </div>
            </div>

            <div v-if="aiConfigs.length === 0" style="text-align: center; padding: 2rem; color: var(--text-muted);">
              No AI configurations registered.
            </div>
          </div>

          <!-- Add AI Config Form -->
          <div class="add-camera-form">
            <h3 class="form-title">Add AI Configuration</h3>
            
            <div class="form-group">
              <label>Configuration Name *</label>
              <input type="text" placeholder="e.g. Gemini 2.5 Flash Primary" v-model="newAiConfig.name">
            </div>

            <div class="form-group">
              <label>Provider Type *</label>
              <select class="filter-select w-full" v-model="newAiConfig.provider_type">
                <option value="gemini">Gemini Vision</option>
                <option value="minimax">Minimax AI</option>
                <option value="mock">Local Mock Provider</option>
              </select>
            </div>

            <div class="form-group" v-if="newAiConfig.provider_type === 'gemini' || newAiConfig.provider_type === 'minimax'">
              <label>API Key *</label>
              <input type="password" :placeholder="'Enter ' + (newAiConfig.provider_type === 'gemini' ? 'Gemini' : 'Minimax') + ' API key'" v-model="newAiConfig.api_key">
            </div>

            <div class="form-group">
              <label>Model Name (optional)</label>
              <input type="text" :placeholder="newAiConfig.provider_type === 'gemini' ? 'e.g. gemini-2.5-flash' : newAiConfig.provider_type === 'minimax' ? 'e.g. MiniMax-M3' : 'e.g. mock-model'" v-model="newAiConfig.model_name">
            </div>

            <div class="form-group" style="flex-direction: row; gap: 0.5rem; align-items: center; margin-top: 0.25rem;">
              <input type="checkbox" id="ai-active-chk" v-model="newAiConfig.is_active">
              <label for="ai-active-chk" style="cursor: pointer; margin-bottom: 0;">Set as Active Configuration</label>
            </div>

            <div v-if="addAiConfigError" class="form-error">
              {{ addAiConfigError }}
            </div>

            <button class="submit-btn" :disabled="isAddingAiConfig" @click="addAiConfig">
              <i data-lucide="plus-circle" style="width: 16px; height: 16px;"></i>
              {{ isAddingAiConfig ? 'Saving Config...' : 'Save AI Configuration' }}
            </button>
          </div>
        </template>
      </aside>

      <!-- Details pane column -->
      <main class="detail-pane">
        <div v-if="selectedEvent" style="display: flex; flex-direction: column; gap: 1.5rem; flex: 1;">
          <!-- Detail Header -->
          <div class="detail-header">
            <div>
              <h2 class="detail-title">Detection Details</h2>
              <div style="font-size: 0.8rem; color: var(--text-muted); margin-top: 0.2rem;">
                ID: {{ selectedEvent.id }}
              </div>
            </div>
            <div class="detail-meta-tags">
              <span class="tag tag-cam">
                <i data-lucide="video" style="width: 14px; height: 14px;"></i>
                Camera: {{ getCameraName(selectedEvent.camera_id) }} ({{ selectedEvent.camera_id || 'N/A' }})
              </span>
              <span class="tag" v-if="selectedEvent.camera_id && getCameraLocation(selectedEvent.camera_id)">
                <i data-lucide="map-pin" style="width: 14px; height: 14px;"></i>
                Location: {{ getCameraLocation(selectedEvent.camera_id) }}
              </span>
              <span class="tag">
                <i data-lucide="clock" style="width: 14px; height: 14px;"></i>
                {{ formatFullDateTime(selectedEvent.created_at) }}
              </span>
            </div>
          </div>

          <!-- Split Panel Content -->
          <div class="detail-grid">
            <!-- Left Split: Image and Plate Card -->
            <div style="display: flex; flex-direction: column; gap: 1.25rem;">
              <!-- Image Frame -->
              <div class="image-container">
                <img :src="'/uploads/' + selectedEvent.image_path" alt="Camera Payload Snapshot">
                <span 
                  class="badge img-overlay-badge" 
                  :class="selectedEvent.is_plate_valid ? 'badge-success' : 'badge-error'"
                >
                  {{ selectedEvent.is_plate_valid ? 'Valid License Plate' : 'Invalid Plate' }}
                </span>
              </div>

              <!-- Styled Plate render -->
              <div class="plate-box-container">
                <div v-if="selectedEvent.is_plate_valid" class="bangla-plate-render">
                  <div class="plate-line-top">
                    <span>{{ selectedEvent.district }}</span>
                    <span v-if="selectedEvent.metro_prefix">মেট্রো</span>
                    <span>{{ selectedEvent.vehicle_class }}</span>
                  </div>
                  <div class="plate-line-bottom">
                    {{ selectedEvent.plate_number }}
                  </div>
                </div>
                <div v-else style="color: var(--status-error); text-align: center;">
                  <i data-lucide="alert-triangle" style="width: 32px; height: 32px; margin-bottom: 0.5rem; display: inline-block;"></i>
                  <p style="font-weight: 600;">Plate verification failed</p>
                  <p style="font-size: 0.8rem; color: var(--text-muted);">The OCR output did not match standard Bangladeshi registration formats.</p>
                </div>

                <div class="plate-meta-info">
                  <div class="meta-item">
                    <div class="meta-item-lbl">District</div>
                    <div class="meta-item-val">{{ selectedEvent.district || '—' }}</div>
                  </div>
                  <div class="meta-item">
                    <div class="meta-item-lbl">Metro Tag</div>
                    <div class="meta-item-val">{{ selectedEvent.metro_prefix ? 'মেট্রো (Metro)' : 'None' }}</div>
                  </div>
                  <div class="meta-item">
                    <div class="meta-item-lbl">Vehicle Class</div>
                    <div class="meta-item-val">{{ selectedEvent.vehicle_class || '—' }}</div>
                  </div>
                  <div class="meta-item">
                    <div class="meta-item-lbl">Plate Number</div>
                    <div class="meta-item-val">{{ selectedEvent.plate_number || '—' }}</div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Right Split: OCR raw logs -->
            <div class="ai-raw-log-container">
              <h3 class="raw-log-title">
                <i data-lucide="cpu" style="color: #10b981; width: 16px; height: 16px;"></i>
                AI Vision Provider Raw Response log
              </h3>
              <div class="raw-log-content">{{ selectedEvent.raw_ai_text || 'No logs generated.' }}</div>
            </div>
          </div>
        </div>

        <!-- Empty State -->
        <div v-else class="empty-state">
          <i data-lucide="camera-off" style="width: 48px; height: 48px;"></i>
          <h3>No Event Selected</h3>
          <p>Select a motion event from the feed to inspect details and plates</p>
        </div>
      </main>
    </div>
  </div>
</template>

<style>
:root {
  --bg-color: #0b0c16;
  --card-bg: rgba(22, 24, 47, 0.65);
  --card-border: rgba(99, 102, 241, 0.15);
  --accent-primary: #6366f1; /* Indigo */
  --accent-glow: rgba(99, 102, 241, 0.4);
  --text-main: #f3f4f6;
  --text-muted: #9ca3af;
  --status-success: #10b981;
  --status-error: #ef4444;
  --status-warning: #f59e0b;
  --plate-bg: #1e293b;
  --plate-border: #f59e0b;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: 'Inter', sans-serif;
  background-color: var(--bg-color);
  color: var(--text-main);
  overflow-x: hidden;
  min-height: 100vh;
  background-image: 
    radial-gradient(circle at 10% 20%, rgba(99, 102, 241, 0.15) 0%, transparent 40%),
    radial-gradient(circle at 90% 80%, rgba(239, 68, 68, 0.08) 0%, transparent 40%);
  background-attachment: fixed;
}

h1, h2, h3, .brand {
  font-family: 'Outfit', sans-serif;
}

/* Layout */
.app-container {
  max-width: 1600px;
  margin: 0 auto;
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  min-height: 100vh;
}

/* Header */
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1rem 1.5rem;
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  border: 1px solid var(--card-border);
  border-radius: 16px;
  box-shadow: 0 8px 32px 0 rgba(0, 0, 0, 0.37);
}

.logo-section {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  text-align: left;
}

.logo-icon {
  background: linear-gradient(135deg, var(--accent-primary), #ec4899);
  width: 42px;
  height: 42px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 0 15px var(--accent-glow);
}

.logo-text {
  font-size: 1.5rem;
  font-weight: 800;
  letter-spacing: 1px;
  background: linear-gradient(to right, #ffffff, #a5b4fc);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.system-stats {
  display: flex;
  gap: 1.5rem;
}

.stat-card {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.stat-val {
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--accent-primary);
  text-shadow: 0 0 10px var(--accent-glow);
}

.stat-lbl {
  font-size: 0.75rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

/* Grid Workspace */
.dashboard-grid {
  display: grid;
  grid-template-columns: 350px 1fr;
  gap: 1.5rem;
  flex: 1;
  min-height: 0; /* Important for scroll */
}

@media (max-width: 1024px) {
  .dashboard-grid {
    grid-template-columns: 1fr;
  }
}

/* Sidebar Column */
.sidebar {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  border: 1px solid var(--card-border);
  border-radius: 16px;
  padding: 1.25rem;
  height: calc(100vh - 140px);
  position: sticky;
  top: 1.5rem;
}

.sidebar-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--card-border);
  padding-bottom: 0.75rem;
}

.sidebar-title {
  font-size: 1.15rem;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

/* Controls / Filtering */
.search-box {
  position: relative;
  width: 100%;
}

.search-box input {
  width: 100%;
  background: rgba(17, 19, 39, 0.8);
  border: 1px solid var(--card-border);
  border-radius: 8px;
  padding: 0.6rem 0.75rem 0.6rem 2.25rem;
  color: var(--text-main);
  font-family: inherit;
  font-size: 0.9rem;
  transition: border-color 0.3s;
}

.search-box input:focus {
  outline: none;
  border-color: var(--accent-primary);
}

.search-box i {
  position: absolute;
  left: 0.75rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-muted);
}

.filter-group {
  display: flex;
  gap: 0.5rem;
}

.filter-select {
  flex: 1;
  background: rgba(17, 19, 39, 0.8);
  border: 1px solid var(--card-border);
  border-radius: 8px;
  padding: 0.5rem;
  color: var(--text-main);
  font-size: 0.85rem;
  outline: none;
}

/* Auto-Refresh Toggle */
.refresh-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: rgba(99, 102, 241, 0.05);
  padding: 0.5rem 0.75rem;
  border-radius: 8px;
  border: 1px dashed rgba(99, 102, 241, 0.2);
  font-size: 0.85rem;
}

.spin-icon {
  width: 14px;
  height: 14px;
  animation: spin 4s linear infinite;
}

.spin-icon.paused {
  animation-play-state: paused;
}

.switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 22px;
}

.switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: #374151;
  transition: .4s;
  border-radius: 34px;
}

.slider:before {
  position: absolute;
  content: "";
  height: 16px;
  width: 16px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: .4s;
  border-radius: 50%;
}

input:checked + .slider {
  background-color: var(--accent-primary);
}

input:checked + .slider:before {
  transform: translateX(22px);
}

/* Events List */
.events-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  overflow-y: auto;
  flex: 1;
  padding-right: 2px;
}

.events-list::-webkit-scrollbar {
  width: 4px;
}
.events-list::-webkit-scrollbar-thumb {
  background: rgba(99, 102, 241, 0.3);
  border-radius: 2px;
}

.event-card {
  background: rgba(17, 19, 39, 0.5);
  border: 1px solid var(--card-border);
  border-radius: 10px;
  padding: 0.85rem;
  cursor: pointer;
  transition: all 0.3s;
  position: relative;
  overflow: hidden;
}

.event-card:hover {
  border-color: var(--accent-primary);
  background: rgba(99, 102, 241, 0.05);
  transform: translateY(-2px);
}

.event-card.active {
  border-color: var(--accent-primary);
  background: rgba(99, 102, 241, 0.12);
  box-shadow: 0 0 15px rgba(99, 102, 241, 0.1);
}

.event-card.active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: linear-gradient(to bottom, var(--accent-primary), #ec4899);
}

.event-card-header {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  color: var(--text-muted);
  margin-bottom: 0.4rem;
}

.event-card-cam {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  font-weight: 500;
}

.event-card-body {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.event-card-plate {
  font-size: 0.95rem;
  font-weight: 700;
  letter-spacing: 0.5px;
}

.badge {
  font-size: 0.65rem;
  font-weight: 700;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  text-transform: uppercase;
}

.badge-success {
  background: rgba(16, 185, 129, 0.15);
  color: var(--status-success);
  border: 1px solid rgba(16, 185, 129, 0.3);
}

.badge-error {
  background: rgba(239, 68, 68, 0.15);
  color: var(--status-error);
  border: 1px solid rgba(239, 68, 68, 0.3);
}

/* Detail Area Column */
.detail-pane {
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  border: 1px solid var(--card-border);
  border-radius: 16px;
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  overflow-y: auto;
  height: calc(100vh - 140px);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-muted);
  gap: 1rem;
}

.empty-state i {
  font-size: 3rem;
  color: rgba(99, 102, 241, 0.2);
}

.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--card-border);
  padding-bottom: 1rem;
}

.detail-title {
  font-size: 1.4rem;
  font-weight: 700;
  text-align: left;
}

.detail-meta-tags {
  display: flex;
  gap: 0.75rem;
}

.tag {
  background: rgba(255, 255, 255, 0.05);
  padding: 0.35rem 0.75rem;
  border-radius: 6px;
  border: 1px solid var(--card-border);
  font-size: 0.8rem;
  display: flex;
  align-items: center;
  gap: 0.35rem;
  color: var(--text-muted);
}

.tag-cam {
  color: var(--text-main);
  border-color: rgba(99, 102, 241, 0.3);
}

/* Detail Split Grid */
.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1.5rem;
  flex: 1;
}

@media (max-width: 768px) {
  .detail-grid {
    grid-template-columns: 1fr;
  }
}

/* Image Display Section */
.image-container {
  position: relative;
  background: #06070d;
  border-radius: 12px;
  border: 1px solid var(--card-border);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  aspect-ratio: 16/9;
  box-shadow: inset 0 0 20px rgba(0,0,0,0.8);
}

.image-container img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.img-overlay-badge {
  position: absolute;
  top: 1rem;
  right: 1rem;
  backdrop-filter: blur(8px);
}

/* Bangla License Plate Card Render */
.plate-box-container {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: center;
  justify-content: center;
  background: rgba(17, 19, 39, 0.6);
  border-radius: 12px;
  padding: 1.5rem;
  border: 1px solid var(--card-border);
}

.bangla-plate-render {
  background-color: var(--plate-bg);
  border: 5px solid var(--plate-border);
  border-radius: 12px;
  padding: 0.75rem 2rem;
  width: 280px;
  text-align: center;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.4), inset 0 0 15px rgba(0, 0, 0, 0.6);
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

/* Indent bolt look */
.bangla-plate-render::before, .bangla-plate-render::after {
  content: '';
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 12px;
  height: 12px;
  background: radial-gradient(circle, #bbb 30%, #555 70%);
  border-radius: 50%;
  box-shadow: 0 2px 4px rgba(0,0,0,0.5);
}
.bangla-plate-render::before { left: 8px; }
.bangla-plate-render::after { right: 8px; }

.plate-line-top {
  font-size: 1.5rem;
  font-weight: 800;
  color: #ffffff;
  letter-spacing: 1px;
  border-bottom: 2px solid rgba(255, 255, 255, 0.2);
  padding-bottom: 0.25rem;
  display: flex;
  justify-content: center;
  gap: 0.5rem;
}

.plate-line-bottom {
  font-size: 2.1rem;
  font-weight: 900;
  color: #ffffff;
  letter-spacing: 2px;
}

.plate-meta-info {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  font-size: 0.85rem;
}

.meta-item {
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  padding: 0.6rem;
  text-align: left;
}

.meta-item-lbl {
  color: var(--text-muted);
  font-size: 0.75rem;
  margin-bottom: 0.2rem;
  text-transform: uppercase;
}

.meta-item-val {
  font-weight: 600;
  color: var(--text-main);
}

/* AI Prompt Raw Log Output */
.ai-raw-log-container {
  background: rgba(6, 7, 13, 0.8);
  border: 1px solid var(--card-border);
  border-radius: 12px;
  padding: 1rem;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.raw-log-title {
  font-size: 0.85rem;
  color: var(--text-muted);
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 0.35rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  padding-bottom: 0.5rem;
  text-align: left;
}

.raw-log-content {
  font-family: 'Courier New', Courier, monospace;
  font-size: 0.85rem;
  color: #10b981; /* green terminals vibe */
  white-space: pre-wrap;
  overflow-y: auto;
  flex: 1;
  line-height: 1.4;
  text-align: left;
}

.raw-log-content::-webkit-scrollbar {
  width: 3px;
}
.raw-log-content::-webkit-scrollbar-thumb {
  background: rgba(16, 185, 129, 0.2);
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* Sidebar Tabs */
.tabs-header {
  display: flex;
  background: rgba(17, 19, 39, 0.8);
  border: 1px solid var(--card-border);
  border-radius: 8px;
  padding: 0.25rem;
  margin-bottom: 0.5rem;
}

.tab-btn {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-muted);
  padding: 0.5rem;
  border-radius: 6px;
  cursor: pointer;
  font-family: inherit;
  font-size: 0.85rem;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
  transition: all 0.3s;
}

.tab-btn:hover {
  color: var(--text-main);
  background: rgba(255, 255, 255, 0.03);
}

.tab-btn.active {
  color: #ffffff;
  background: var(--accent-primary);
  box-shadow: 0 0 10px var(--accent-glow);
}

/* Cameras Manager Styles */
.cameras-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  overflow-y: auto;
  flex: 1;
  padding-right: 2px;
}

.cameras-list::-webkit-scrollbar {
  width: 4px;
}
.cameras-list::-webkit-scrollbar-thumb {
  background: rgba(99, 102, 241, 0.3);
  border-radius: 2px;
}

.camera-card {
  background: rgba(17, 19, 39, 0.5);
  border: 1px solid var(--card-border);
  border-radius: 10px;
  padding: 0.85rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  text-align: left;
}

.camera-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.camera-card-name {
  font-weight: 700;
  font-size: 0.95rem;
  display: flex;
  align-items: center;
  gap: 0.35rem;
  color: var(--text-main);
}

.delete-cam-btn {
  background: transparent;
  border: none;
  color: var(--status-error);
  cursor: pointer;
  opacity: 0.7;
  transition: opacity 0.3s;
  padding: 0.25rem;
  border-radius: 4px;
}

.delete-cam-btn:hover {
  opacity: 1;
  background: rgba(239, 68, 68, 0.1);
}

.camera-card-body {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  font-size: 0.8rem;
}

.camera-card-info {
  display: flex;
  gap: 0.35rem;
}

.camera-card-info .lbl {
  color: var(--text-muted);
  font-weight: 500;
}

.camera-card-info .val {
  color: var(--text-main);
}

/* Add Camera Form */
.add-camera-form {
  border-top: 1px solid var(--card-border);
  padding-top: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  text-align: left;
}

.form-title {
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--text-main);
  margin-bottom: 0.25rem;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.form-group label {
  font-size: 0.75rem;
  color: var(--text-muted);
  font-weight: 500;
}

.form-group input {
  background: rgba(17, 19, 39, 0.8);
  border: 1px solid var(--card-border);
  border-radius: 6px;
  padding: 0.5rem 0.75rem;
  color: var(--text-main);
  font-family: inherit;
  font-size: 0.85rem;
  outline: none;
  transition: border-color 0.3s;
}

.form-group input:focus {
  border-color: var(--accent-primary);
}

.form-error {
  color: var(--status-error);
  font-size: 0.75rem;
  font-weight: 600;
}

.submit-btn {
  background: var(--accent-primary);
  border: none;
  border-radius: 6px;
  color: white;
  padding: 0.6rem;
  font-family: inherit;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
  transition: all 0.3s;
  box-shadow: 0 0 10px var(--accent-glow);
}

.submit-btn:hover:not(:disabled) {
  opacity: 0.9;
  box-shadow: 0 0 15px var(--accent-glow);
}

.submit-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.w-full {
  width: 100%;
}

.camera-filter-box {
  margin-bottom: 0.5rem;
}
</style>

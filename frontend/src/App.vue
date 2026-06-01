<script setup>
import { ref, onMounted } from 'vue'
import axios from 'axios'

const events = ref([])
const loading = ref(true)

const fetchEvents = async () => {
  try {
    const response = await axios.get('http://localhost:3000/api/events')
    events.value = response.data
  } catch (error) {
    console.error("Error fetching events:", error)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  fetchEvents()
})
</script>

<template>
  <main>
    <h1>CCTV Motion Events Dashboard</h1>

    <div v-if="loading">Loading events...</div>

    <div v-else class="events-grid">
      <div v-for="event in events" :key="event.id" class="card">
        <img :src="`http://localhost:3000/${event.image_path}`" alt="Event Image" class="event-image" />
        <div class="card-content">
          <p><strong>Time:</strong> {{ new Date(event.created_at).toLocaleString() }}</p>
          <p><strong>Description:</strong> {{ event.ai_description || 'N/A' }}</p>
          <p><strong>License Plate:</strong> {{ event.license_plate || 'None detected' }}</p>
          <p>
            <strong>Validation:</strong>
            <span :class="{'valid': event.is_valid_plate, 'invalid': !event.is_valid_plate}">
              {{ event.is_valid_plate ? 'Valid' : 'Invalid' }}
            </span>
          </p>
        </div>
      </div>
    </div>
  </main>
</template>

<style scoped>
main {
  max-width: 1200px;
  margin: 0 auto;
  padding: 2rem;
  font-family: sans-serif;
}

h1 {
  text-align: center;
  margin-bottom: 2rem;
}

.events-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 2rem;
}

.card {
  border: 1px solid #ccc;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 2px 5px rgba(0,0,0,0.1);
  display: flex;
  flex-direction: column;
}

.event-image {
  width: 100%;
  height: 200px;
  object-fit: cover;
  background-color: #f0f0f0;
}

.card-content {
  padding: 1rem;
}

.card-content p {
  margin: 0.5rem 0;
  font-size: 0.9rem;
}

.valid {
  color: green;
  font-weight: bold;
}

.invalid {
  color: red;
  font-weight: bold;
}
</style>

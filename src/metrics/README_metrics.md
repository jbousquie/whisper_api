# Module de métriques de l'API Whisper

## Architecture générale du système de métriques

Le système de métriques de l'API Whisper est conçu selon une architecture modulaire qui permet de collecter, traiter et exporter des données de performance vers différents systèmes de monitoring. Le module principal se trouve dans `metrics` et utilise un pattern de traits pour supporter plusieurs backends d'export.

Sont supportés les produits suivants :

- Prometheus
- StatsD
- null

En prévision/liste à faire ?

- Grafana
- NewRelic (K8's, complexe, pas de crate, via OpenTelemetry)
- InfluxDB
- Graphite (redondant avec statsd)
- Datadog (nécessite une clef d'API)

## Structure du module de métriques

### Module Principal (`metrics.rs`)

Le cœur du système est la structure `Metrics` qui implémente le trait `MetricsCollector`. Cette structure :

1. Collecte des métriques asynchrones : toutes les opérations de métriques sont non-bloquantes grâce à l'utilisation de *tokio*
2. Thread-safe : utilise `Arc<dyn  MetricsCollector + Send  + Sync>` pour permettre le partage entre threads
3. Gestion d'erreurs robuste : intègre des validations de sécurité pour éviter les débordements numériques

#### Types de métriques supportées

- Compteurs : pour les événements cumulatifs (jobs soumis, complétés, annulés)
- Jauges : pour les valeurs instantanées (taille de la queue, jobs en cours)
- Histogrammes : pour les distributions de temps (durée des transcriptions)

#### Intégration avec l'API Whisper

Le système de métriques s'intègre à plusieurs niveaux dans l'application :

 1. Dans le gestionnaire de queue (`queue_manager.rs`)

`

// Enregistrement de la soumission d'un job
self.metrics.record_job_submitted(&job_model, &job_language).await;

// Mise à jour de la taille de la queue
self.metrics.set_queue_size(safe_size).await;

// Suivi du nombre de jobs en traitement
self.metrics.set_jobs_processing(safe_count).await;

// Enregistrement de la completion d'un job
metrics.record_job_completed(&job.model, &job.language, duration, "success").await;
`
2. Dans l'application principale (`main.rs`)

#### Métriques HTTP pour les requêtes API

- Exposition d'un endpoint `/metrics` pour Prometheus
- Configuration du backend de métriques selon les variables d'environnement

## Les différents modules d'export

### Module Prometheus (`prometheus.rs`)

Le module Prometheus.rs implémente l'export vers le système de monitoring Prometheus.

#### Fonctionnalités

- Registre de métriques : Utilise `prometheus::Registry` pour organiser toutes les métriques
- Types de métriques Prometheus :
- CounterVec : pour les compteurs avec labels (modèle, langue)
- GaugeVec : pour les jauges avec dimensions
- HistogramVec : pour les distributions temporelles avec buckets configurables

#### Métriques exposées

- whisper_jobs_submitted_total : nombre total de jobs soumis
- whisper_jobs_completed_total : nombre total de jobs complétés
- whisper_queue_size : taille actuelle de la queue
- whisper_jobs_processing : nombre de jobs en cours de traitement
- whisper_job_duration_seconds : histogramme des durées de transcription

#### Configuration

Buckets d'histogramme personnalisables via les variables d'environnement
Labels automatiques pour le modèle et la langue
Exposition via endpoint HTTP /metrics

### Module StatsD (`statsd.rs`)

Le module StatsD permet l'envoi de métriques vers des serveurs StatsD compatibles.

#### Protocole

- UDP : envoi non-bloquant des métriques via UDP
- Format StatsD : respect du protocole standard `metric_name:value|type|@sample_rate`
- Types supportés : c (counter), g (gauge), h (histogram)

#### Fonctionnalités

- Envoi asynchrone : utilise `tokio::net::UdpSocket` pour l'envoi non-bloquant
- Gestion d'erreurs : logging des erreurs sans interrompre l'application
- Formatage automatique : conversion des métriques au format StatsD

#### Configuration

Le fichier `STATSD.md` fournit un guide complet d'intégration avec différents backends (Graphite, InfluxDB, DataDog).

### Module Null (`null.rs`)

Le module Null implémente un backend "no-op" pour les métriques.

#### Utilité

- Environnements de test : désactive les métriques sans modifier le code
- Performance : évite la surcharge des métriques en production si non nécessaires
- Développement : permet de développer sans infrastructure de monitoring

#### Implémentation

No op...
  
#### Configuration et sélection du backend

Le système utilise des variables d'environnement pour sélectionner le backend

`
// Configuration automatique basée sur l'environnement
let metrics = match std::env::var("METRICS_BACKEND").as_deref() {
    Ok("prometheus") => Metrics::new_prometheus(),
    Ok("statsd") => Metrics::new_statsd(),
    _ => Metrics::new_null(), // Backend par défaut
};`

1. Avantages du système
    - Modularité : facilite l'ajout de nouveaux backends de métriques
    - Performance : opérations asynchrones non-bloquantes
    - Fiabilité : gestion d'erreurs robuste qui n'impacte pas l'application principale
    - Flexibilité : support de multiples systèmes de monitoring simultanément
    - Observabilité : métriques détaillées sur tous les aspects de l'API

2. Métriques collectées

Le système surveille en temps réel :

    - Débit : nombre de jobs soumis, traités, échoués
    - Latence : temps de traitement par modèle et langue
    - Charge : taille de la queue, jobs actifs
    - Qualité : taux de succès, types d'erreurs
    - Ressources : utilisation selon la configuration de concurrence

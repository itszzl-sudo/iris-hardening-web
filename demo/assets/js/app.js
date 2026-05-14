// Nexus Tech - Enterprise Website
(function() {
    // Smooth scroll for nav links
    document.querySelectorAll('a[href^="#"]').forEach(function(a) {
        a.addEventListener('click', function(e) {
            e.preventDefault();
            var target = document.querySelector(this.getAttribute('href'));
            if (target) target.scrollIntoView({ behavior: 'smooth' });
        });
    });

    // Navbar background on scroll
    var navbar = document.querySelector('.navbar');
    window.addEventListener('scroll', function() {
        if (window.scrollY > 50) {
            navbar.style.background = 'rgba(26,26,46,0.98)';
        } else {
            navbar.style.background = 'rgba(26,26,46,0.95)';
        }
    });

    // Animated counter for stats
    var observed = false;
    var observer = new IntersectionObserver(function(entries) {
        if (entries[0].isIntersecting && !observed) {
            observed = true;
            document.querySelectorAll('.stat-number').forEach(function(el) {
                var text = el.textContent;
                var match = text.match(/([\d,]+\.?\d*)/);
                if (match) {
                    var target = parseFloat(match[1].replace(/,/g, ''));
                    var suffix = text.replace(match[1], '');
                    var current = 0;
                    var step = target / 40;
                    var interval = setInterval(function() {
                        current += step;
                        if (current >= target) {
                            current = target;
                            clearInterval(interval);
                        }
                        el.textContent = (target >= 100 ? Math.round(current).toLocaleString() : current.toFixed(2)) + suffix;
                    }, 30);
                }
            });
        }
    });
    var statsEl = document.querySelector('.stats-grid');
    if (statsEl) observer.observe(statsEl);

    console.log('[Nexus Tech] Enterprise website loaded');
})();

FROM nginx:1.27-alpine
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY site/ /usr/share/nginx/html/
RUN v=$(md5sum /usr/share/nginx/html/style.css | cut -c1-12) \
    && find /usr/share/nginx/html -name '*.html' -exec sed -i "s#href=\"/style.css\"#href=\"/style.css?v=$v\"#" {} + \
    && grep -q "style.css?v=$v" /usr/share/nginx/html/index.html
EXPOSE 80
